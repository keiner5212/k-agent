import { invoke } from "@tauri-apps/api/core";
import type { SelectedModel } from "@/types/chat";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";

export type AppGenerationKind = "improvePrompt";

export type AppGenerationInput = {
  kind: AppGenerationKind;
  content?: string;
};

export const resolveAppGenerationModel = (): SelectedModel | null => {
  const { appGenerationModel } = useSettingsStore.getState();
  if (appGenerationModel) return appGenerationModel;
  return useSelectionStore.getState().selection;
};

// The backend gives up on its own. This is the belt to that braces: the
// button must never stay dead because a provider never answered.
const REQUEST_TIMEOUT_MS = 150_000;

const withTimeout = <T>(work: Promise<T>, ms: number): Promise<T> => {
  let timer = 0;
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = window.setTimeout(() => reject(new Error("timeout")), ms);
  });
  return Promise.race([work, timeout]).finally(() => window.clearTimeout(timer));
};

export const generateAppContent = async (
  input: AppGenerationInput,
): Promise<{ text?: string; error?: string }> => {
  if (!isTauri()) return { error: DESKTOP_REQUIRED };
  const model = resolveAppGenerationModel();
  if (!model) return { error: "noModel" };

  try {
    const result = await withTimeout(
      invoke<{ text: string }>("generate_app_content", {
        input: {
          providerId: model.providerId,
          modelId: model.modelId,
          kind: input.kind,
          content: input.content ?? "",
          limitProviderDataUse: useSettingsStore.getState().limitProviderDataUse,
        },
      }),
      REQUEST_TIMEOUT_MS,
    );
    const text = result.text.trim();
    if (!text) return { error: "empty" };
    return { text };
  } catch (error) {
    const message = ipcErrorMessage(error);
    return { error: message === "timeout" ? "timeout" : message };
  }
};
