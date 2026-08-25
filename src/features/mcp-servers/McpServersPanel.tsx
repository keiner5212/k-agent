import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { Table, type TableColumn } from "@/components/Table";
import { Toggle } from "@/components/Toggle";
import { highlightMatch } from "@/lib/highlight";
import { useMcpServersStore } from "@/lib/mcp-servers";
import type { McpServer } from "@/types/mcp-servers";
import { DeleteMcpServerDialog } from "./DeleteMcpServerDialog";
import { McpServerFormDialog } from "./McpServerFormDialog";
import { McpServerToolsDialog } from "./McpServerToolsDialog";

type McpServersPanelProps = {
  query: string;
};

const targetLabel = (server: McpServer): string => {
  if (server.transport === "stdio") {
    const args = server.args?.length ? ` ${server.args.join(" ")}` : "";
    return `${server.command ?? ""}${args}`.trim();
  }
  return server.url ?? "";
};

const formatToolsCell = (
  server: McpServer,
  t: (key: string, options?: Record<string, unknown>) => string,
): string => {
  const tools = server.tools ?? [];
  if (tools.length > 0) return tools.map((tool) => tool.name).join(", ");
  if (server.toolsProbeError) return server.toolsProbeError;
  return t("mcpServers.tools.none");
};

const ToolsCell = ({
  server,
  query,
  t,
  onOpen,
}: {
  server: McpServer;
  query: string;
  t: (key: string, options?: Record<string, unknown>) => string;
  onOpen: (server: McpServer) => void;
}): ReactNode => {
  const tools = server.tools ?? [];
  const label = formatToolsCell(server, t);
  if (tools.length === 0) return highlightMatch(label, query);
  return (
    <button
      type="button"
      className="data-table__tools-link"
      aria-label={t("mcpServers.toolsDialog.open", { name: server.name })}
      onClick={() => onOpen(server)}
    >
      {highlightMatch(label, query)}
    </button>
  );
};

export const McpServersPanel = ({ query }: McpServersPanelProps): ReactNode => {
  const { t } = useTranslation();
  const servers = useMcpServersStore((state) => state.servers);
  const loading = useMcpServersStore((state) => state.loading);
  const error = useMcpServersStore((state) => state.error);
  const load = useMcpServersStore((state) => state.load);
  const refreshTools = useMcpServersStore((state) => state.refreshTools);
  const setEnabled = useMcpServersStore((state) => state.setEnabled);
  const remove = useMcpServersStore((state) => state.remove);
  const [editTarget, setEditTarget] = useState<McpServer | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<McpServer | null>(null);
  const [toolsTarget, setToolsTarget] = useState<McpServer | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [togglingId, setTogglingId] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    if (query.length === 0) return servers;
    const needle = query.toLowerCase();
    return servers.filter((server) => {
      const haystack = [
        server.name,
        server.transport,
        server.command ?? "",
        server.url ?? "",
        ...(server.args ?? []),
        ...(server.tools?.map((tool) => tool.name) ?? []),
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
    });
  }, [query, servers]);

  const columns = useMemo(
    (): TableColumn<McpServer>[] => [
      {
        id: "name",
        header: t("mcpServers.table.name"),
        className: "data-table__name",
        cellProps: (server) => ({ title: server.name }),
        render: (server) => highlightMatch(server.name, query),
      },
      {
        id: "transport",
        header: t("mcpServers.table.transport"),
        className: "data-table__tokens",
        render: (server) => t(`mcpServers.transports.${server.transport}`),
      },
      {
        id: "target",
        header: t("mcpServers.table.target"),
        className: "data-table__desc",
        cellProps: (server) => ({ title: targetLabel(server) }),
        render: (server) => highlightMatch(targetLabel(server), query),
      },
      {
        id: "tools",
        header: t("mcpServers.table.tools"),
        className: "data-table__tools",
        wrap: true,
        cellProps: (server) => ({
          title: formatToolsCell(server, t),
          "data-empty": (server.tools?.length ?? 0) === 0 ? "true" : undefined,
        }),
        render: (server) => (
          <ToolsCell server={server} query={query} t={t} onOpen={setToolsTarget} />
        ),
      },
      {
        id: "enabled",
        header: t("mcpServers.table.enabled"),
        className: "data-table__toggle",
        render: (server) => (
          <Toggle
            checked={server.enabled}
            onChange={(next) => {
              setTogglingId(server.id);
              void setEnabled(server.id, next).finally(() => setTogglingId(null));
            }}
            label={t("mcpServers.form.enabled")}
            showLabel={false}
            disabled={togglingId === server.id}
          />
        ),
      },
      {
        id: "actions",
        header: <span className="visually-hidden">{t("mcpServers.table.actions")}</span>,
        className: "data-table__actions",
        render: (server) => (
          <>
            <IconButton
              label={t("mcpServers.actions.refreshTools")}
              onClick={() => {
                setRefreshingId(server.id);
                void refreshTools(server.id).finally(() => setRefreshingId(null));
              }}
              disabled={refreshingId === server.id}
            >
              <RefreshCw
                size={12}
                strokeWidth={1.5}
                className={refreshingId === server.id ? "spin" : undefined}
              />
            </IconButton>
            <IconButton label={t("mcpServers.actions.edit")} onClick={() => setEditTarget(server)}>
              <Pencil size={12} strokeWidth={1.5} />
            </IconButton>
            <IconButton
              label={t("mcpServers.actions.delete")}
              onClick={() => setDeleteTarget(server)}
            >
              <Trash2 size={12} strokeWidth={1.5} />
            </IconButton>
          </>
        ),
      },
    ],
    [query, refreshTools, refreshingId, setEnabled, togglingId, t],
  );

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{highlightMatch(t("mcpServers.title"), query)}</h2>
        <p className="section__description">{highlightMatch(t("mcpServers.description"), query)}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("mcpServers.actions.refresh")}
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
            {t("mcpServers.actions.add")}
          </GlassButton>
        </div>
        {visible.length === 0 && !loading ? (
          <p className="skill-context__empty">{t("mcpServers.empty")}</p>
        ) : (
          <Table
            columns={columns}
            rows={visible}
            rowKey={(server) => server.id}
            layout="fixed"
            stickyHeader
            scrollable
            cellAlign="top"
          />
        )}
      </div>

      <McpServerFormDialog open={createOpen} onOpenChange={setCreateOpen} />

      <McpServerFormDialog
        open={editTarget !== null}
        draft={editTarget ?? undefined}
        onOpenChange={(open) => {
          if (!open) setEditTarget(null);
        }}
      />

      <DeleteMcpServerDialog
        open={deleteTarget !== null}
        serverName={deleteTarget?.name ?? ""}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget) return "missing target";
          const result = await remove(deleteTarget.id);
          return result.error;
        }}
      />

      <McpServerToolsDialog
        open={toolsTarget !== null}
        server={toolsTarget}
        onOpenChange={(open) => {
          if (!open) setToolsTarget(null);
        }}
      />
    </section>
  );
};
