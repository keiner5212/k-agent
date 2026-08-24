import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2, Pencil, Plug, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useProvidersStore, providerKindLabel } from "@/lib/providers";
import type { Provider } from "@/types/providers";
import { ProviderForm } from "./ProviderForm";

type ProviderListProps = {
  onClose: () => void;
};

const formatSyncedAt = (timestamp: number | undefined, locale: string): string => {
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

const ProviderCard = ({
  provider,
  locale,
  onEdit,
  onRefresh,
  onDelete,
}: {
  provider: Provider;
  locale: string;
  onEdit: () => void;
  onRefresh: () => Promise<void>;
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

  const synced = formatSyncedAt(provider.lastSyncedAt, locale);

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
          <ul className="provider-card__model-list">
            {provider.models.map((model) => (
              <li key={model}>{model}</li>
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

export const ProviderList = ({ onClose }: ProviderListProps): ReactNode => {
  const { t, i18n } = useTranslation();
  const providers = useProvidersStore((state) => state.providers);
  const loading = useProvidersStore((state) => state.loading);
  const load = useProvidersStore((state) => state.load);
  const remove = useProvidersStore((state) => state.remove);
  const refresh = useProvidersStore((state) => state.refresh);
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
          <Plug size={20} strokeWidth={1.5} />
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

      <div className="dialog-footer" style={{ marginTop: "var(--space-2)" }}>
        <button type="button" className="btn btn--ghost" onClick={onClose}>
          {t("providers.close")}
        </button>
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
