import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { Table, type TableColumn } from "@/components/Table";
import { highlightMatch } from "@/lib/highlight";
import { SLASH_COMMANDS } from "@/lib/slash-commands";
import { useSettingsStore } from "@/lib/settings";
import { PROMPT_COMMAND_NAME, type PromptCommand } from "@/types/settings";
import { CommandFormDialog } from "./CommandFormDialog";
import { DeleteCommandDialog } from "./DeleteCommandDialog";

type CommandsPanelProps = {
  query: string;
};

type CommandRow = {
  key: string;
  name: string;
  description: string;
  kindKey: "settings.commands.kind.function" | "settings.commands.kind.prompt";
  custom: PromptCommand | null;
};

const matches = (text: string, query: string): boolean =>
  query.length === 0 || text.toLowerCase().includes(query.toLowerCase());

export const CommandsPanel = ({ query }: CommandsPanelProps): ReactNode => {
  const { t } = useTranslation();
  const commands = useSettingsStore((state) => state.promptCommands);
  const setPromptCommands = useSettingsStore((state) => state.setPromptCommands);
  const [formMode, setFormMode] = useState<"create" | "edit" | null>(null);
  const [formNonce, setFormNonce] = useState(0);
  const [editing, setEditing] = useState<PromptCommand | null>(null);
  const [deleteName, setDeleteName] = useState<string | null>(null);

  const rows = useMemo((): CommandRow[] => {
    const builtin: CommandRow[] = SLASH_COMMANDS.map((command) => ({
      key: `builtin:${command.id}`,
      name: command.name,
      description: t(command.descriptionKey),
      kindKey:
        command.kind === "action"
          ? "settings.commands.kind.function"
          : "settings.commands.kind.prompt",
      custom: null,
    }));
    const custom: CommandRow[] = commands.map((command) => ({
      key: `custom:${command.name}`,
      name: command.name,
      description: command.description || command.template,
      kindKey: "settings.commands.kind.prompt",
      custom: command,
    }));
    return [...builtin, ...custom].filter((row) =>
      matches(`${row.name} ${row.description} ${t(row.kindKey)}`, query),
    );
  }, [commands, query, t]);

  const columns = useMemo(
    (): TableColumn<CommandRow>[] => [
      {
        id: "name",
        header: t("settings.commands.table.name"),
        className: "data-table__name",
        render: (row) => highlightMatch(`/${row.name}`, query),
      },
      {
        id: "description",
        header: t("settings.commands.table.description"),
        className: "data-table__desc",
        wrap: true,
        cellProps: (row) => ({ title: row.description }),
        render: (row) => highlightMatch(row.description, query),
      },
      {
        id: "kind",
        header: t("settings.commands.table.kind"),
        render: (row) => t(row.kindKey),
      },
      {
        id: "actions",
        header: <span className="visually-hidden">{t("settings.commands.table.actions")}</span>,
        className: "data-table__actions",
        render: (row) =>
          row.custom ? (
            <>
              <IconButton
                label={t("settings.commands.actions.edit")}
                onClick={() => {
                  setEditing(row.custom);
                  setFormNonce((value) => value + 1);
                  setFormMode("edit");
                }}
              >
                <Pencil size={12} strokeWidth={1.5} />
              </IconButton>
              <IconButton
                label={t("settings.commands.actions.delete")}
                onClick={() => setDeleteName(row.name)}
              >
                <Trash2 size={12} strokeWidth={1.5} />
              </IconButton>
            </>
          ) : null,
      },
    ],
    [query, t],
  );

  const save = (next: PromptCommand): string | undefined => {
    const taken = new Set([
      ...SLASH_COMMANDS.map((command) => command.name.toLowerCase()),
      ...commands.map((command) => command.name.toLowerCase()),
    ]);
    if (editing) taken.delete(editing.name.toLowerCase());
    if (!PROMPT_COMMAND_NAME.test(next.name) || taken.has(next.name.toLowerCase())) {
      return t("settings.commands.nameTaken");
    }
    if (!next.template) return t("settings.commands.templateRequired");
    const list = editing
      ? commands.map((command) => (command.name === editing.name ? next : command))
      : [...commands, next];
    setPromptCommands(list);
    return undefined;
  };

  return (
    <section className="commands-panel">
      <header className="commands-panel__head">
        <h2 className="section__heading">
          {highlightMatch(t("settings.sections.commands"), query)}
        </h2>
        <p className="section__description">
          {highlightMatch(t("settings.commands.description"), query)}
        </p>
        <GlassButton
          variant="primary"
          className="commands-panel__add"
          onClick={() => {
            setEditing(null);
            setFormNonce((value) => value + 1);
            setFormMode("create");
          }}
        >
          <Plus strokeWidth={1.5} />
          {t("settings.commands.add")}
        </GlassButton>
      </header>
      {rows.length === 0 ? (
        <div className="settings-empty">{t("settings.searchEmpty")}</div>
      ) : (
        <Table
          columns={columns}
          rows={rows}
          rowKey={(row) => row.key}
          layout="fixed"
          stickyHeader
          scrollable
          cellAlign="top"
        />
      )}
      <CommandFormDialog
        key={formNonce}
        open={formMode !== null}
        mode={formMode ?? "create"}
        initial={editing}
        onOpenChange={(open) => {
          if (!open) {
            setFormMode(null);
            setEditing(null);
          }
        }}
        onSubmit={save}
      />
      <DeleteCommandDialog
        open={deleteName !== null}
        name={deleteName ?? ""}
        onOpenChange={(open) => {
          if (!open) setDeleteName(null);
        }}
        onConfirm={() => {
          if (!deleteName) return;
          setPromptCommands(commands.filter((command) => command.name !== deleteName));
        }}
      />
    </section>
  );
};
