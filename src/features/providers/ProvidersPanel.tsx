import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { Table, type TableColumn } from "@/components/Table";
import { highlightMatch } from "@/lib/highlight";
import { useProvidersStore } from "@/lib/providers";
import type { ModelInfo, Provider } from "@/types/providers";
import { DeleteProviderDialog } from "./DeleteProviderDialog";
import { ModelFormDialog } from "./ModelFormDialog";
import { ProviderFormDialog } from "./ProviderFormDialog";
import { ProviderModelsDialog } from "./ProviderModelsDialog";

type ProvidersPanelProps = {
  query: string;
};

type ModelEditorState = {
  providerId: string;
  model?: ModelInfo;
};

const ModelsCell = ({
  provider,
  t,
  onOpen,
}: {
  provider: Provider;
  t: (key: string, options?: Record<string, unknown>) => string;
  onOpen: (provider: Provider) => void;
}): ReactNode => {
  const count = provider.models.length;
  const label = t("providers.modelsCount", { count });
  return (
    <button
      type="button"
      className="data-table__tools-link"
      aria-label={t("providers.modelsDialog.open", { name: provider.name, count })}
      onClick={() => onOpen(provider)}
    >
      {label}
    </button>
  );
};

export const ProvidersPanel = ({ query }: ProvidersPanelProps): ReactNode => {
  const { t } = useTranslation();
  const providers = useProvidersStore((state) => state.providers);
  const loading = useProvidersStore((state) => state.loading);
  const error = useProvidersStore((state) => state.error);
  const load = useProvidersStore((state) => state.load);
  const remove = useProvidersStore((state) => state.remove);
  const refresh = useProvidersStore((state) => state.refresh);
  const upsertModel = useProvidersStore((state) => state.upsertModel);
  const removeModel = useProvidersStore((state) => state.removeModel);
  const [createOpen, setCreateOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<Provider | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Provider | null>(null);
  const [modelsTargetId, setModelsTargetId] = useState<string | null>(null);
  const [modelEditor, setModelEditor] = useState<ModelEditorState | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    if (query.length === 0) return providers;
    const needle = query.toLowerCase();
    return providers.filter((provider) => {
      const haystack = [
        provider.name,
        provider.kind,
        t(`providers.kinds.${provider.kind}`),
        provider.baseUrl,
        ...provider.models.flatMap((model) => [model.id, model.displayName ?? ""]),
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
    });
  }, [providers, query, t]);

  const modelsProvider = modelsTargetId
    ? (providers.find((provider) => provider.id === modelsTargetId) ?? null)
    : null;

  const editingProvider = modelEditor
    ? providers.find((provider) => provider.id === modelEditor.providerId)
    : undefined;

  const columns = useMemo(
    (): TableColumn<Provider>[] => [
      {
        id: "name",
        header: t("providers.table.name"),
        className: "data-table__name",
        cellProps: (provider) => ({ title: provider.name }),
        render: (provider) => highlightMatch(provider.name, query),
      },
      {
        id: "kind",
        header: t("providers.table.kind"),
        className: "data-table__kind",
        render: (provider) => t(`providers.kinds.${provider.kind}`),
      },
      {
        id: "url",
        header: t("providers.table.url"),
        className: "data-table__desc",
        cellProps: (provider) => ({ title: provider.baseUrl }),
        render: (provider) => highlightMatch(provider.baseUrl, query),
      },
      {
        id: "models",
        header: t("providers.table.models"),
        className: "data-table__models",
        cellProps: (provider) => ({
          "data-empty": provider.models.length === 0 ? "true" : undefined,
        }),
        render: (provider) => (
          <ModelsCell provider={provider} t={t} onOpen={(next) => setModelsTargetId(next.id)} />
        ),
      },
      {
        id: "actions",
        header: <span className="visually-hidden">{t("providers.table.actions")}</span>,
        className: "data-table__actions",
        render: (provider) => (
          <>
            <IconButton
              label={t("providers.actions.refresh")}
              onClick={() => {
                setRefreshingId(provider.id);
                void refresh(provider.id).finally(() => setRefreshingId(null));
              }}
              disabled={refreshingId === provider.id}
            >
              <RefreshCw
                size={12}
                strokeWidth={1.5}
                className={refreshingId === provider.id ? "spin" : undefined}
              />
            </IconButton>
            <IconButton label={t("providers.actions.edit")} onClick={() => setEditTarget(provider)}>
              <Pencil size={12} strokeWidth={1.5} />
            </IconButton>
            <IconButton
              label={t("providers.actions.delete")}
              onClick={() => setDeleteTarget(provider)}
            >
              <Trash2 size={12} strokeWidth={1.5} />
            </IconButton>
          </>
        ),
      },
    ],
    [query, refresh, refreshingId, t],
  );

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{highlightMatch(t("providers.title"), query)}</h2>
        <p className="section__description">{highlightMatch(t("providers.description"), query)}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("providers.actions.refreshList")}
          onClick={() => void load()}
          disabled={loading}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}

      <div className="skill-context">
        <div className="skill-context__header">
          <GlassButton
            variant="primary"
            className="skill-context__new"
            onClick={() => setCreateOpen(true)}
          >
            <Plus strokeWidth={1.5} />
            {t("providers.add")}
          </GlassButton>
        </div>
        {visible.length === 0 && !loading ? (
          <p className="skill-context__empty">{t("providers.empty")}</p>
        ) : (
          <Table
            columns={columns}
            rows={visible}
            rowKey={(provider) => provider.id}
            layout="fixed"
            stickyHeader
            scrollable
            cellAlign="top"
          />
        )}
      </div>

      <ProviderFormDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onSaved={() => void load()}
      />

      <ProviderFormDialog
        open={editTarget !== null}
        draft={editTarget ?? undefined}
        onOpenChange={(open) => {
          if (!open) setEditTarget(null);
        }}
        onSaved={() => void load()}
      />

      <DeleteProviderDialog
        open={deleteTarget !== null}
        providerName={deleteTarget?.name ?? ""}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget) return "missing target";
          const result = await remove(deleteTarget.id);
          if (result.error) return result.error;
          await load();
          return undefined;
        }}
      />

      <ProviderModelsDialog
        open={modelsProvider !== null}
        provider={modelsProvider}
        onOpenChange={(open) => {
          if (!open) setModelsTargetId(null);
        }}
        onAddModel={() => {
          if (!modelsTargetId) return;
          setModelEditor({ providerId: modelsTargetId });
        }}
        onEditModel={(model) => {
          if (!modelsTargetId) return;
          setModelEditor({ providerId: modelsTargetId, model });
        }}
      />

      <ModelFormDialog
        open={modelEditor !== null}
        model={modelEditor?.model}
        onOpenChange={(open) => {
          if (!open) setModelEditor(null);
        }}
        onSave={async (draft) => {
          if (!modelEditor) return "missing provider";
          const result = await upsertModel(modelEditor.providerId, draft);
          if (result.error) return result.error;
          await load();
          return undefined;
        }}
        onDelete={
          modelEditor?.model && editingProvider
            ? async () => {
                const result = await removeModel(editingProvider.id, modelEditor.model?.id ?? "");
                if (result.error) return result.error;
                await load();
                return undefined;
              }
            : undefined
        }
      />
    </section>
  );
};
