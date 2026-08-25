import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { useProvidersStore } from "@/lib/providers";
import { selectEffort, useSelectionStore } from "@/lib/selected-model";
import type { ModelInfo } from "@/types/providers";
import type { SelectedModel } from "@/types/chat";

const resolveModel = (
  providers: ReturnType<typeof useProvidersStore.getState>["providers"],
  selection: SelectedModel | null,
): ModelInfo | null => {
  if (!selection) return null;
  const provider = providers.find((p) => p.id === selection.providerId);
  if (!provider) return null;
  return provider.models.find((m) => m.id === selection.modelId) ?? null;
};

export const EffortSelector = (): ReactNode => {
  const { t } = useTranslation();
  const selection = useSelectionStore((state) => state.selection);
  const effort = useSelectionStore(selectEffort);
  const setEffort = useSelectionStore((state) => state.setEffort);
  const providers = useProvidersStore((state) => state.providers);
  const model = resolveModel(providers, selection);
  const effortLevels = model?.effortLevels ?? [];

  if (!model || effortLevels.length === 0) return null;

  return (
    <div className="effort-selector">
      <Select
        id="model-bar-effort"
        value={effort ?? effortLevels[0]}
        onChange={(next) => setEffort(next)}
        options={effortLevels.map((level) => ({ value: level, label: level }))}
        ariaLabel={t("chat.effort.label")}
      />
    </div>
  );
};
