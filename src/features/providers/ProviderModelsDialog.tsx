import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Plus, Star } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { highlightMatch } from "@/lib/highlight";
import { useProvidersStore } from "@/lib/providers";
import { formatContextWindow, type ModelInfo, type Provider } from "@/types/providers";

type ProviderModelsDialogProps = {
  open: boolean;
  provider: Provider | null;
  onOpenChange: (open: boolean) => void;
  onAddModel: () => void;
  onEditModel: (model: ModelInfo) => void;
};

const MODALITY_KEY: Record<string, string> = {
  text: "providers.model.modalities.text",
  image: "providers.model.modalities.image",
  audio: "providers.model.modalities.audio",
  video: "providers.model.modalities.video",
  pdf: "providers.model.modalities.pdf",
};

const uniqueModalities = (values: string[] | undefined): string[] => {
  if (!values) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const value of values) {
    const key = value.trim().toLowerCase();
    if (!MODALITY_KEY[key] || seen.has(key)) continue;
    seen.add(key);
    out.push(key);
  }
  return out;
};

const sortModels = (models: ModelInfo[]): ModelInfo[] =>
  [...models].sort(
    (left, right) => Number(Boolean(right.favorite)) - Number(Boolean(left.favorite)),
  );

const modelMatches = (model: ModelInfo, needle: string): boolean => {
  if (needle.length === 0) return true;
  const haystack = [model.id, model.displayName ?? "", model.family ?? ""].join(" ").toLowerCase();
  return haystack.includes(needle);
};

export const ProviderModelsDialog = ({
  open,
  provider,
  onOpenChange,
  onAddModel,
  onEditModel,
}: ProviderModelsDialogProps): ReactNode => {
  if (!open || !provider) return null;
  return (
    <ProviderModelsBody
      key={provider.id}
      provider={provider}
      onOpenChange={onOpenChange}
      onAddModel={onAddModel}
      onEditModel={onEditModel}
    />
  );
};

type ProviderModelsBodyProps = {
  provider: Provider;
  onOpenChange: (open: boolean) => void;
  onAddModel: () => void;
  onEditModel: (model: ModelInfo) => void;
};

const ProviderModelsBody = ({
  provider,
  onOpenChange,
  onAddModel,
  onEditModel,
}: ProviderModelsBodyProps): ReactNode => {
  const { t } = useTranslation();
  const setFavorite = useProvidersStore((state) => state.setFavorite);
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const frame = requestAnimationFrame(() => searchRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const needle = query.trim().toLowerCase();
  const visible = useMemo(
    () => sortModels(provider.models).filter((model) => modelMatches(model, needle)),
    [needle, provider.models],
  );

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      titleKey="providers.modelsDialog.title"
      placement="center"
      size="default"
      footer={
        <GlassButton variant="secondary" onClick={() => onOpenChange(false)}>
          {t("providers.modelsDialog.close")}
        </GlassButton>
      }
    >
      <div className="provider-models-dialog">
        <p className="provider-models-dialog__provider">
          {t("providers.modelsDialog.provider", { name: provider.name })}
        </p>
        <div className="provider-models-dialog__toolbar">
          <input
            ref={searchRef}
            type="search"
            className="input provider-models-dialog__search"
            placeholder={t("providers.modelsDialog.search")}
            aria-label={t("providers.modelsDialog.search")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <GlassButton variant="primary" onClick={onAddModel}>
            <Plus strokeWidth={1.5} />
            {t("providers.actions.addModel")}
          </GlassButton>
        </div>
        {visible.length > 0 ? (
          <ul className="model-list">
            {visible.map((model) => (
              <ModelRow
                key={model.id}
                model={model}
                query={needle}
                onEdit={() => onEditModel(model)}
                onToggleFavorite={async () => {
                  await setFavorite(provider.id, model.id, !model.favorite);
                }}
              />
            ))}
          </ul>
        ) : (
          <p className="provider-models-dialog__empty">
            {provider.models.length === 0
              ? t("providers.noModels")
              : t("providers.modelsDialog.noMatches")}
          </p>
        )}
      </div>
    </Dialog>
  );
};

const ModelRow = ({
  model,
  query,
  onEdit,
  onToggleFavorite,
}: {
  model: ModelInfo;
  query: string;
  onEdit: () => void;
  onToggleFavorite: () => Promise<void>;
}): ReactNode => {
  const { t } = useTranslation();

  const context = formatContextWindow(model.contextWindow);
  const hasContext = model.contextWindow !== undefined;
  const hasOutput = model.maxOutputTokens !== undefined;
  const display = model.displayName ?? model.id;

  const inputModalities = uniqueModalities(model.input);
  const outputModalities = uniqueModalities(model.output).filter(
    (modality) => !inputModalities.includes(modality),
  );

  return (
    <li className="model-row">
      <div className="model-row__head">
        <span className="model-row__id">{highlightMatch(model.id, query)}</span>
        {inputModalities.map((modality) => (
          <span
            key={`in-${modality}`}
            className="model-row__chip"
            title={`${t("providers.model.input")}: ${t(MODALITY_KEY[modality])}`}
          >
            {t(MODALITY_KEY[modality])}
          </span>
        ))}
        {outputModalities.map((modality) => (
          <span
            key={`out-${modality}`}
            className="model-row__chip model-row__chip--muted"
            title={`${t("providers.model.outputModalities")}: ${t(MODALITY_KEY[modality])}`}
          >
            {t(MODALITY_KEY[modality])}
          </span>
        ))}
        {model.reasoning ? (
          <span className="model-row__chip" title={t("providers.model.capability.reasoning")}>
            {t("providers.model.capability.reasoning")}
          </span>
        ) : null}
        {model.toolCall ? (
          <span className="model-row__chip" title={t("providers.model.capability.toolCall")}>
            {t("providers.model.capability.toolCall")}
          </span>
        ) : null}
        {model.structuredOutput ? (
          <span
            className="model-row__chip"
            title={t("providers.model.capability.structuredOutput")}
          >
            {t("providers.model.capability.structuredOutput")}
          </span>
        ) : null}
        {model.attachment ? (
          <span className="model-row__chip" title={t("providers.model.capability.attachment")}>
            {t("providers.model.capability.attachment")}
          </span>
        ) : null}
        {model.source === "custom" ? (
          <span className="model-row__chip">{t("providers.model.custom")}</span>
        ) : null}
        {model.userEdited && model.source !== "custom" ? (
          <span className="model-row__chip">{t("providers.model.edited")}</span>
        ) : null}
        {model.favorite ? (
          <span className="model-row__chip">{t("providers.model.favorite")}</span>
        ) : null}
      </div>
      {display !== model.id ? (
        <div className="model-row__name">{highlightMatch(display, query)}</div>
      ) : null}
      <div className="model-row__meta">
        <span>
          <span className="model-row__meta-label">{t("providers.model.context")}</span>
          <span className="model-row__meta-value">{hasContext ? context : "-"}</span>
        </span>
        <span>
          <span className="model-row__meta-label">{t("providers.model.output")}</span>
          <span className="model-row__meta-value">
            {hasOutput ? formatContextWindow(model.maxOutputTokens) : "-"}
          </span>
        </span>
        <IconButton
          className="model-row__refresh"
          label={
            model.favorite ? t("providers.actions.unfavorite") : t("providers.actions.favorite")
          }
          onClick={() => void onToggleFavorite()}
        >
          <Star
            size={12}
            strokeWidth={1.5}
            fill={model.favorite ? "currentColor" : "none"}
            className={model.favorite ? "model-row__star model-row__star--on" : "model-row__star"}
          />
        </IconButton>
        <IconButton label={t("providers.actions.edit")} onClick={onEdit}>
          <Pencil size={12} strokeWidth={1.5} />
        </IconButton>
      </div>
    </li>
  );
};
