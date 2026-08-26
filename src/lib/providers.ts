import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { ModelDraft, Provider, ProviderDraft, ProviderErrorPayload } from "@/types/providers";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import { acquireWorkerCores, getWorkerCoreSnapshot } from "@/lib/worker-cores";

export type ProviderMutationResult = {
  provider?: Provider;
  error?: string;
  errorPayload?: ProviderErrorPayload;
};

const parseProviderError = (error: unknown): ProviderErrorPayload | undefined => {
  if (!error || typeof error !== "object") return undefined;
  const record = error as Record<string, unknown>;
  const kind = record.kind;
  if (typeof kind !== "string") return undefined;
  const message = record.message;
  switch (kind) {
    case "timeout":
      return {
        kind: "timeout",
        seconds: typeof record.seconds === "number" ? record.seconds : 0,
      };
    case "api":
      if (typeof record.status !== "number" || typeof message !== "string") return undefined;
      return { kind: "api", status: record.status, message };
    case "path":
    case "io":
    case "parse":
    case "http":
    case "unreachable":
    case "invalidUrl":
    case "notFound":
    case "duplicate":
    case "crypto":
      if (typeof message !== "string") return undefined;
      return { kind, message };
    default:
      return undefined;
  }
};

const payloadToMessage = (payload: ProviderErrorPayload): string => {
  if (payload.kind === "timeout") return `timeout after ${payload.seconds}s`;
  if (payload.kind === "api") return payload.message || `server returned ${payload.status}`;
  return payload.message;
};

const replaceProvider = (providers: Provider[], provider: Provider): Provider[] => {
  const idx = providers.findIndex((item) => item.id === provider.id);
  if (idx < 0) return [...providers, provider];
  const next = [...providers];
  next[idx] = provider;
  return next;
};

const withMaxCores = async <T>(label: string, run: (cores: number) => Promise<T>): Promise<T> => {
  const lease = acquireWorkerCores(label, getWorkerCoreSnapshot().limit);
  try {
    return await run(lease.cores);
  } finally {
    lease.release();
  }
};

const runMutation = async (
  work: () => Promise<Provider | undefined>,
): Promise<ProviderMutationResult> => {
  if (!isTauri()) return { error: DESKTOP_REQUIRED };
  try {
    const provider = await work();
    return provider ? { provider } : {};
  } catch (error) {
    const payload = parseProviderError(error);
    if (payload) {
      return { error: payloadToMessage(payload), errorPayload: payload };
    }
    return { error: ipcErrorMessage(error) };
  }
};

type ProvidersStore = {
  providers: Provider[];
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  save: (draft: ProviderDraft) => Promise<ProviderMutationResult>;
  remove: (id: string) => Promise<ProviderMutationResult>;
  refresh: (id: string) => Promise<ProviderMutationResult>;
  upsertModel: (providerId: string, draft: ModelDraft) => Promise<ProviderMutationResult>;
  removeModel: (providerId: string, modelId: string) => Promise<ProviderMutationResult>;
  setFavorite: (
    providerId: string,
    modelId: string,
    favorite: boolean,
  ) => Promise<ProviderMutationResult>;
};

export const useProvidersStore = create<ProvidersStore>((set) => ({
  providers: [],
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ providers: [], loading: false });
      return;
    }
    try {
      const providers = await invoke<Provider[]>("list_providers");
      set({ providers, loading: false });
    } catch (error) {
      set({ loading: false, error: ipcErrorMessage(error) });
    }
  },

  save: async (draft) =>
    runMutation(async () =>
      withMaxCores("saveProvider", async (workerCores) => {
        const provider = await invoke<Provider>("save_provider", {
          input: {
            id: draft.id ?? null,
            name: draft.name,
            kind: draft.kind,
            baseUrl: draft.baseUrl,
            apiKey: draft.apiKey ?? null,
            clearApiKey: Boolean(draft.clearApiKey),
            workerCores,
          },
        });
        set((state) => ({
          providers: replaceProvider(state.providers, provider),
          error: undefined,
        }));
        return provider;
      }),
    ),

  remove: async (id) =>
    runMutation(async () => {
      await invoke("delete_provider", { id });
      set((state) => ({
        providers: state.providers.filter((item) => item.id !== id),
      }));
      return undefined;
    }),

  refresh: async (id) =>
    runMutation(async () =>
      withMaxCores("refreshProviderModels", async (workerCores) => {
        const provider = await invoke<Provider>("refresh_provider_models", { id, workerCores });
        set((state) => ({ providers: replaceProvider(state.providers, provider) }));
        return provider;
      }),
    ),

  upsertModel: async (providerId, draft) =>
    runMutation(async () => {
      const provider = await invoke<Provider>("upsert_provider_model", {
        input: {
          providerId,
          originalId: draft.originalId ?? null,
          id: draft.id,
          displayName: draft.displayName ?? null,
          family: draft.family ?? null,
          contextWindow: draft.contextWindow ?? null,
          maxOutputTokens: draft.maxOutputTokens ?? null,
          multimodal: draft.multimodal,
          effortLevels: draft.effortLevels ?? null,
        },
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),

  removeModel: async (providerId, modelId) =>
    runMutation(async () => {
      const provider = await invoke<Provider>("delete_provider_model", {
        id: providerId,
        modelId,
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),

  setFavorite: async (providerId, modelId, favorite) =>
    runMutation(async () => {
      const provider = await invoke<Provider>("set_model_favorite", {
        input: { id: providerId, modelId, favorite },
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),
}));
