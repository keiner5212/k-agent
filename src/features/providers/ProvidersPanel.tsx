import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  Copy,
  Eye,
  EyeOff,
  Loader2,
  Pencil,
  Plug,
  Plus,
  RefreshCw,
  Star,
  Trash2,
} from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useProvidersStore } from "@/lib/providers";
import {
  formatContextWindow,
  type ModelDraft,
  type ModelInfo,
  type Provider,
} from "@/types/providers";
import { ModelForm } from "./ModelForm";
import { ProviderForm } from "./ProviderForm";

export const ProvidersPanel = (): ReactNode => {
  const { t } = useTranslation();
  const load = useProvidersStore((state) => state.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("providers.title")}</h2>
        <p className="section__description">{t("providers.description")}</p>
      </header>
      <ProvidersList />
    </section>
  );
};

const sortModels = (models: ModelInfo[]): ModelInfo[] =>
  [...models].sort(
    (left, right) => Number(Boolean(right.favorite)) - Number(Boolean(left.favorite)),
  );

const ModelRow = ({
  model,
  onRefresh,
  onEdit,
  onAddVariant,
  onToggleFavorite,
}: {
  model: ModelInfo;
  onRefresh: () => Promise<void>;
  onEdit: () => void;
  onAddVariant: () => void;
  onToggleFavorite: () => Promise<void>;
}): ReactNode => {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);

  const handleRefresh = async (): Promise<void> => {
    setBusy(true);
    try {
      await onRefresh();
    } finally {
      setBusy(false);
    }
  };

  const context = formatContextWindow(model.contextWindow);
  const hasContext = model.contextWindow !== undefined;
  const hasOutput = model.maxOutputTokens !== undefined;
  const display = model.displayName ?? model.id;

  return (
    <li className="model-row">
      <div className="model-row__head">
        <span className="model-row__id">{model.id}</span>
        {model.multimodal ? (
          <span className="model-row__chip" title={t("providers.model.multimodal")}>
            <Eye size={10} strokeWidth={1.5} />
            <span>{t("providers.model.multimodal")}</span>
          </span>
        ) : (
          <span
            className="model-row__chip model-row__chip--muted"
            title={t("providers.model.textOnly")}
          >
            <EyeOff size={10} strokeWidth={1.5} />
            <span>{t("providers.model.textOnly")}</span>
          </span>
        )}
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
      {display !== model.id ? <div className="model-row__name">{display}</div> : null}
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
        <IconButton
          className="model-row__refresh"
          label={t("providers.actions.refreshModel")}
          onClick={() => void handleRefresh()}
          disabled={busy}
        >
          {busy ? (
            <Loader2 size={12} strokeWidth={1.5} className="spin" />
          ) : (
            <RefreshCw size={12} strokeWidth={1.5} />
          )}
        </IconButton>
        <IconButton label={t("providers.actions.addVariant")} onClick={onAddVariant}>
          <Copy size={12} strokeWidth={1.5} />
        </IconButton>
        <IconButton label={t("providers.actions.edit")} onClick={onEdit}>
          <Pencil size={12} strokeWidth={1.5} />
        </IconButton>
      </div>
    </li>
  );
};

const ProviderCard = ({
  provider,
  locale,
  onEdit,
  onRefresh,
  onRefreshModel,
  onDelete,
}: {
  provider: Provider;
  locale: string;
  onEdit: () => void;
  onRefresh: () => Promise<void>;
  onRefreshModel: (modelId: string) => Promise<void>;
  onDelete: () => Promise<void>;
}): ReactNode => {
  const { t } = useTranslation();
  const upsertModel = useProvidersStore((state) => state.upsertModel);
  const removeModel = useProvidersStore((state) => state.removeModel);
  const setFavorite = useProvidersStore((state) => state.setFavorite);
  const [refreshing, setRefreshing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editor, setEditor] = useState<{
    model?: ModelInfo;
    familyPreset?: string;
  } | null>(null);

  const handleRefresh = async (): Promise<void> => {
    setRefreshing(true);
    try {
      await onRefresh();
    } finally {
      setRefreshing(false);
    }
  };

  const handleDelete = async (): Promise<void> => {
    setDeleting(true);
    try {
      await onDelete();
    } finally {
      setDeleting(false);
    }
  };

  const formatSyncedAt = (timestamp: number | undefined): string => {
    if (!timestamp) return "";
    try {
      return new Intl.DateTimeFormat(locale, {
        dateStyle: "short",
        timeStyle: "short",
      }).format(new Date(timestamp * 1000));
    } catch {
      return "";
    }
  };

  const synced = formatSyncedAt(provider.lastSyncedAt);

  return (
    <li className="provider-card">
      <div className="provider-card__header">
        <div className="provider-card__title">
          <span className="provider-card__name">{provider.name}</span>
          <span className="provider-card__badge">{t(`providers.kinds.${provider.kind}`)}</span>
        </div>
        <div className="provider-card__actions">
          <IconButton
            label={t("providers.actions.refresh")}
            onClick={() => void handleRefresh()}
            disabled={refreshing || deleting}
          >
            {refreshing ? (
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
            ) : (
              <RefreshCw size={14} strokeWidth={1.5} />
            )}
          </IconButton>
          <IconButton label={t("providers.actions.edit")} onClick={onEdit} disabled={deleting}>
            <Pencil size={14} strokeWidth={1.5} />
          </IconButton>
          <IconButton
            label={t("providers.actions.delete")}
            onClick={() => void handleDelete()}
            disabled={refreshing || deleting}
          >
            {deleting ? (
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
            ) : (
              <Trash2 size={14} strokeWidth={1.5} />
            )}
          </IconButton>
        </div>
      </div>
      <div className="provider-card__url">{provider.baseUrl}</div>
      <div className="provider-card__meta">
        <span>{t("providers.modelsCount", { count: provider.models.length })}</span>
        {synced ? <span className="provider-card__synced">{synced}</span> : null}
      </div>
      {editor ? (
        <ModelForm
          key={editor.model?.id ?? editor.familyPreset ?? "new"}
          model={editor.model}
          familyPreset={editor.familyPreset}
          onCancel={() => setEditor(null)}
          onSave={async (draft: ModelDraft) => {
            const result = await upsertModel(provider.id, draft);
            if (result.error) return result.error;
            setEditor(null);
            return undefined;
          }}
          onDelete={
            editor.model
              ? async () => {
                  const result = await removeModel(provider.id, editor.model?.id ?? "");
                  if (result.error) return result.error;
                  setEditor(null);
                  return undefined;
                }
              : undefined
          }
        />
      ) : (
        <details className="provider-card__models" open={provider.models.length > 0}>
          <summary>{t("providers.viewModels")}</summary>
          <div className="model-list-actions">
            <GlassButton variant="ghost" onClick={() => setEditor({})}>
              <Plus size={14} strokeWidth={1.5} />
              <span>{t("providers.actions.addModel")}</span>
            </GlassButton>
          </div>
          {provider.models.length > 0 ? (
            <ul className="model-list">
              {sortModels(provider.models).map((model) => (
                <ModelRow
                  key={model.id}
                  model={model}
                  onRefresh={async () => {
                    await onRefreshModel(model.id);
                  }}
                  onEdit={() => setEditor({ model })}
                  onAddVariant={() =>
                    setEditor({
                      familyPreset: model.family ?? model.id,
                    })
                  }
                  onToggleFavorite={async () => {
                    await setFavorite(provider.id, model.id, !model.favorite);
                  }}
                />
              ))}
            </ul>
          ) : (
            <div className="provider-card__meta">
              <span className="provider-card__muted">{t("providers.noModels")}</span>
            </div>
          )}
        </details>
      )}
    </li>
  );
};

const ProvidersList = (): ReactNode => {
  const { t, i18n } = useTranslation();
  const providers = useProvidersStore((state) => state.providers);
  const loading = useProvidersStore((state) => state.loading);
  const load = useProvidersStore((state) => state.load);
  const remove = useProvidersStore((state) => state.remove);
  const refresh = useProvidersStore((state) => state.refresh);
  const refreshModel = useProvidersStore((state) => state.refreshModel);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [adding, setAdding] = useState(false);

  if (editing || adding) {
    return (
      <ProviderForm
        draft={editing ?? undefined}
        onCancel={() => {
          setEditing(null);
          setAdding(false);
        }}
        onSaved={() => {
          setEditing(null);
          setAdding(false);
          void load();
        }}
      />
    );
  }

  return (
    <>
      {providers.length === 0 ? (
        <div className="provider-empty">
          <Plug size={18} strokeWidth={1.5} />
          <p>{t("providers.empty")}</p>
        </div>
      ) : (
        <ul className="provider-list">
          {providers.map((provider) => (
            <ProviderCard
              key={provider.id}
              provider={provider}
              locale={i18n.language}
              onEdit={() => setEditing(provider)}
              onRefresh={async () => {
                await refresh(provider.id);
              }}
              onRefreshModel={async (modelId) => {
                await refreshModel(provider.id, modelId);
              }}
              onDelete={async () => {
                await remove(provider.id);
              }}
            />
          ))}
        </ul>
      )}

      {loading ? (
        <div className="provider-loading">
          <Loader2 size={14} strokeWidth={1.5} className="spin" />
          <span>{t("providers.loading")}</span>
        </div>
      ) : null}

      <div className="provider-actions">
        <GlassButton variant="primary" onClick={() => setAdding(true)} disabled={loading}>
          <Plus size={14} strokeWidth={1.5} />
          <span>{t("providers.add")}</span>
        </GlassButton>
      </div>
    </>
  );
};
