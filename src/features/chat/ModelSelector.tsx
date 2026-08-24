import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, Cpu } from "lucide-react";
import { useProvidersStore } from "@/lib/providers";
import { useSelectionStore } from "@/lib/selected-model";
import { formatContextWindow, type ModelInfo, type Provider } from "@/types/providers";

type ProviderGroup = {
  provider: Provider;
  favorites: ModelInfo[];
};

const groupsFor = (providers: Provider[]): ProviderGroup[] => {
  const groups: ProviderGroup[] = [];
  for (const provider of providers) {
    const favorites = provider.models.filter((model) => model.favorite);
    if (favorites.length === 0) continue;
    groups.push({ provider, favorites });
  }
  return groups;
};

export const ModelSelector = (): ReactNode => {
  const { t } = useTranslation();
  const providers = useProvidersStore((state) => state.providers);
  const selection = useSelectionStore((state) => state.selection);
  const select = useSelectionStore((state) => state.select);
  const load = useProvidersStore((state) => state.load);

  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!open) return;
    const handler = (event: MouseEvent): void => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", handler);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", handler);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const groups = useMemo(() => groupsFor(providers), [providers]);
  const selectedModel = useMemo(() => {
    if (!selection) return null;
    const provider = providers.find((item) => item.id === selection.providerId);
    if (!provider) return null;
    const model = provider.models.find((item) => item.id === selection.modelId);
    if (!model) return null;
    return { provider, model };
  }, [providers, selection]);

  const handlePick = (providerId: string, modelId: string): void => {
    select({ providerId, modelId });
    setOpen(false);
  };

  return (
    <div className="model-selector" ref={rootRef}>
      <button
        type="button"
        className="model-selector__pill"
        data-empty={selectedModel ? "false" : "true"}
        onClick={() => setOpen((prev) => !prev)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t("chat.modelSelector.label")}
      >
        <Cpu size={14} strokeWidth={1.5} />
        <span className="model-selector__pill-label">
          {selectedModel
            ? `${selectedModel.provider.name} / ${selectedModel.model.displayName ?? selectedModel.model.id}`
            : t("chat.modelSelector.empty")}
        </span>
        {selectedModel?.model.contextWindow !== undefined ? (
          <span className="model-selector__pill-context">
            {formatContextWindow(selectedModel.model.contextWindow)}
          </span>
        ) : null}
        <ChevronDown size={12} strokeWidth={1.5} />
      </button>

      {open ? (
        <div className="model-selector__popover" role="listbox">
          {groups.length === 0 ? (
            <p className="model-selector__empty">{t("chat.modelSelector.noFavorites")}</p>
          ) : (
            groups.map((group) => (
              <div className="model-selector__group" key={group.provider.id}>
                <span className="model-selector__group-label">{group.provider.name}</span>
                <ul className="model-selector__list">
                  {group.favorites.map((model) => {
                    const isSelected =
                      selection?.providerId === group.provider.id &&
                      selection?.modelId === model.id;
                    return (
                      <li key={model.id}>
                        <button
                          type="button"
                          className="model-selector__item"
                          data-selected={isSelected ? "true" : "false"}
                          onClick={() => handlePick(group.provider.id, model.id)}
                        >
                          <span className="model-selector__item-name">
                            {model.displayName ?? model.id}
                          </span>
                          {model.contextWindow !== undefined ? (
                            <span className="model-selector__item-context">
                              {formatContextWindow(model.contextWindow)}
                            </span>
                          ) : null}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
};
