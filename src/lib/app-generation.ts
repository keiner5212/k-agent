import { invoke } from "@tauri-apps/api/core";
import type { SelectedModel } from "@/types/chat";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";

export type AppGenerationKind = "improvePrompt" | "composeSkill" | "composePersonality";

export type AppGenerationInput = {
  kind: AppGenerationKind;
  content?: string;
  name?: string;
  description?: string;
};

export const resolveAppGenerationModel = (): SelectedModel | null => {
  const { appGenerationModel } = useSettingsStore.getState();
  if (appGenerationModel) return appGenerationModel;
  return useSelectionStore.getState().selection;
};

export const generateAppContent = async (
  input: AppGenerationInput,
): Promise<{ text?: string; error?: string }> => {
  if (!isTauri()) return { error: DESKTOP_REQUIRED };
  const model = resolveAppGenerationModel();
  if (!model) return { error: "noModel" };

  try {
    const result = await invoke<{ text: string }>("generate_app_content", {
      input: {
        providerId: model.providerId,
        modelId: model.modelId,
        kind: input.kind,
        content: input.content ?? "",
        name: input.name ?? "",
        description: input.description ?? "",
      },
    });
    const text = result.text.trim();
    if (!text) return { error: "empty" };
    return { text };
  } catch (error) {
    return { error: ipcErrorMessage(error) };
  }
};
