import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { isTauri } from "@/lib/platform";
import type { SelectedModel } from "@/types/chat";
import type { ModelRequestOverride } from "@/types/model-request";
import type { Provider } from "@/types/providers";

const STORE_FILE = "settings.json";
const KEY_SELECTED_MODEL = "selectedModel";
const KEY_EFFORT_PREFIX = "modelEffort:";
const KEY_REQUEST_PREFIX = "modelRequest:";

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const effortKey = (selection: SelectedModel): string =>
  `${KEY_EFFORT_PREFIX}${selection.providerId}:${selection.modelId}`;

const requestKey = (selection: SelectedModel): string =>
  `${KEY_REQUEST_PREFIX}${selection.providerId}:${selection.modelId}`;

const emptyOverride = (): ModelRequestOverride => ({});

const sanitizeOverride = (value: unknown): ModelRequestOverride => {
  if (!value || typeof value !== "object") return emptyOverride();
  const raw = value as Record<string, unknown>;
  const next: ModelRequestOverride = {};
  if (typeof raw.reasoningMode === "string" && raw.reasoningMode.trim()) {
    next.reasoningMode = raw.reasoningMode.trim();
  }
  if (typeof raw.effort === "string" && raw.effort.trim()) next.effort = raw.effort.trim();
  if (typeof raw.serviceTier === "string" && raw.serviceTier.trim()) {
    next.serviceTier = raw.serviceTier.trim();
  }
  if (typeof raw.temperature === "number" && Number.isFinite(raw.temperature)) {
    next.temperature = raw.temperature;
  }
  if (
    typeof raw.maxOutputTokens === "number" &&
    Number.isFinite(raw.maxOutputTokens) &&
    raw.maxOutputTokens > 0
  ) {
    next.maxOutputTokens = Math.round(raw.maxOutputTokens);
  }
  return next;
};

type SelectionState = {
  selection: SelectedModel | null;
  effortByModel: Record<string, string>;
  requestByModel: Record<string, ModelRequestOverride>;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  select: (next: SelectedModel | null) => void;
  setEffort: (effort: string | null) => void;
  setRequest: (next: ModelRequestOverride) => void;
  reconcileWithProviders: (providers: readonly Provider[]) => void;
};

const selectionStillValid = (selection: SelectedModel, providers: readonly Provider[]): boolean =>
  providers.some(
    (provider) =>
      provider.id === selection.providerId &&
      provider.models.some((model) => model.id === selection.modelId),
  );

const pickDefaultSelection = (providers: readonly Provider[]): SelectedModel | null => {
  for (const provider of providers) {
    const favorite = provider.models.find((model) => model.favorite);
    if (favorite) return { providerId: provider.id, modelId: favorite.id };
  }
  for (const provider of providers) {
    const first = provider.models[0];
    if (first) return { providerId: provider.id, modelId: first.id };
  }
  return null;
};

const sameSelection = (left: SelectedModel | null, right: SelectedModel | null): boolean => {
  if (left === right) return true;
  if (!left || !right) return false;
  return left.providerId === right.providerId && left.modelId === right.modelId;
};

const persistSelection = async (selection: SelectedModel | null): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(KEY_SELECTED_MODEL, selection);
    await getStore().save();
  } catch (error) {
    console.warn("selected model persist failed", error);
  }
};

const persistRequest = async (
  selection: SelectedModel,
  request: ModelRequestOverride,
): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(requestKey(selection), request);
    await getStore().save();
  } catch (error) {
    console.warn("model request persist failed", error);
  }
};

const readRequest = async (selection: SelectedModel): Promise<ModelRequestOverride> => {
  const stored = await getStore().get<unknown>(requestKey(selection));
  const request = sanitizeOverride(stored);
  if (request.effort) return request;
  const legacyEffort = await getStore().get<string>(effortKey(selection));
  if (legacyEffort) return { ...request, effort: legacyEffort };
  return request;
};

const persistEffort = async (selection: SelectedModel, effort: string): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(effortKey(selection), effort);
    await getStore().save();
  } catch (error) {
    console.warn("model effort persist failed", error);
  }
};

export const useSelectionStore = create<SelectionState>((set, get) => ({
  selection: null,
  effortByModel: {},
  requestByModel: {},
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      set({ hydrated: true });
      return;
    }
    try {
      const store = getStore();
      const selection = (await store.get<SelectedModel | null>(KEY_SELECTED_MODEL)) ?? null;
      const effortByModel: Record<string, string> = {};
      const requestByModel: Record<string, ModelRequestOverride> = {};
      if (selection) {
        const storedEffort = await store.get<string>(effortKey(selection));
        if (storedEffort) {
          effortByModel[effortKey(selection)] = storedEffort;
        }
        requestByModel[requestKey(selection)] = await readRequest(selection);
      }
      set({ selection, effortByModel, requestByModel, hydrated: true });
    } catch (error) {
      console.warn("selection hydrate failed", error);
      set({ hydrated: true });
    }
  },

  select: (next) => {
    set({ selection: next });
    void persistSelection(next);
    if (!next || !isTauri()) return;
    void readRequest(next).then((request) => {
      set((state) => ({
        requestByModel: { ...state.requestByModel, [requestKey(next)]: request },
      }));
    });
  },

  setEffort: (effort) => {
    const selection = get().selection;
    if (!selection) return;
    const key = effortKey(selection);
    set((state) => {
      const next = { ...state.effortByModel };
      if (effort === null) {
        delete next[key];
      } else {
        next[key] = effort;
      }
      return { effortByModel: next };
    });
    if (effort !== null) void persistEffort(selection, effort);
  },

  setRequest: (next) => {
    const selection = get().selection;
    if (!selection) return;
    const request = sanitizeOverride(next);
    set((state) => ({
      requestByModel: { ...state.requestByModel, [requestKey(selection)]: request },
    }));
    void persistRequest(selection, request);
  },

  reconcileWithProviders: (providers) => {
    const current = get().selection;
    if (current && selectionStillValid(current, providers)) return;
    const next = pickDefaultSelection(providers);
    if (sameSelection(current, next)) return;
    set({ selection: next });
    void persistSelection(next);
  },
}));

export const selectEffort = (state: SelectionState): string | null => {
  const selection = state.selection;
  if (!selection) return null;
  return state.effortByModel[effortKey(selection)] ?? null;
};

export const selectRequest = (state: SelectionState): ModelRequestOverride => {
  const selection = state.selection;
  if (!selection) return emptyOverride();
  return state.requestByModel[requestKey(selection)] ?? emptyOverride();
};
