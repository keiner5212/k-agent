import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, Loader2, Pencil, Plug, Plus, RefreshCw, Trash2 } from "lucide-react";
import { providerKindLabel, useProvidersStore } from "@/lib/providers";
import { formatContextWindow, type ModelInfo, type Provider } from "@/types/providers";
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

const ModelRow = ({
  model,
  onRefresh,
}: {
  model: ModelInfo;
  onRefresh: () => Promise<void>;
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
        <button
          type="button"
          className="icon-button model-row__refresh"
          aria-label={t("providers.actions.refreshModel")}
          title={t("providers.actions.refreshModel")}
          onClick={() => void handleRefresh()}
          disabled={busy}
        >
          {busy ? (
            <Loader2 size={12} strokeWidth={1.5} className="spin" />
          ) : (
            <RefreshCw size={12} strokeWidth={1.5} />
          )}
        </button>
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
  const [refreshing, setRefreshing] = useState(false);
  const [deleting, setDeleting] = useState(false);

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
          <span className="provider-card__badge">{providerKindLabel(provider.kind)}</span>
        </div>
        <div className="provider-card__actions">
          <button
            type="button"
            className="icon-button"
            aria-label={t("providers.actions.refresh")}
            title={t("providers.actions.refresh")}
            onClick={() => void handleRefresh()}
            disabled={refreshing || deleting}
          >
            {refreshing ? (
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
            ) : (
              <RefreshCw size={14} strokeWidth={1.5} />
            )}
          </button>
          <button
            type="button"
            className="icon-button"
            aria-label={t("providers.actions.edit")}
            title={t("providers.actions.edit")}
            onClick={onEdit}
            disabled={deleting}
          >
            <Pencil size={14} strokeWidth={1.5} />
          </button>
          <button
            type="button"
            className="icon-button"
            aria-label={t("providers.actions.delete")}
            title={t("providers.actions.delete")}
            onClick={() => void handleDelete()}
            disabled={refreshing || deleting}
          >
            {deleting ? (
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
            ) : (
              <Trash2 size={14} strokeWidth={1.5} />
            )}
          </button>
        </div>
      </div>
      <div className="provider-card__url">{provider.baseUrl}</div>
      <div className="provider-card__meta">
        <span>{t("providers.modelsCount", { count: provider.models.length })}</span>
        {synced ? <span className="provider-card__synced">{synced}</span> : null}
      </div>
      {provider.models.length > 0 ? (
        <details className="provider-card__models">
          <summary>{t("providers.viewModels")}</summary>
          <ul className="model-list">
            {provider.models.map((model) => (
              <ModelRow
                key={model.id}
                model={model}
                onRefresh={async () => {
                  await onRefreshModel(model.id);
                }}
              />
            ))}
          </ul>
        </details>
      ) : (
        <div className="provider-card__meta">
          <span className="provider-card__muted">{t("providers.noModels")}</span>
        </div>
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
        <button
          type="button"
          className="btn btn--primary"
          onClick={() => setAdding(true)}
          disabled={loading}
        >
          <Plus size={14} strokeWidth={1.5} />
          <span>{t("providers.add")}</span>
        </button>
      </div>
    </>
  );
};
