import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import { useMcpServersStore } from "@/lib/mcp-servers";
import { clearStoredSecretMask, isStoredSecretMask, storedSecretDisplay } from "@/lib/secret-field";
import { MCP_TRANSPORTS, type McpServer, type McpTransport } from "@/types/mcp-servers";

type McpServerFormDialogProps = {
  open: boolean;
  draft?: McpServer;
  onOpenChange: (open: boolean) => void;
};

const splitArgs = (value: string): string[] =>
  value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);

const parseKvLines = (value: string): Record<string, string> => {
  const out: Record<string, string> = {};
  for (const line of value.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const idx = trimmed.indexOf("=");
    if (idx <= 0) continue;
    const key = trimmed.slice(0, idx).trim();
    const val = trimmed.slice(idx + 1).trim();
    if (key) out[key] = val;
  }
  return out;
};

const resolveKvSecrets = (
  lines: string,
  hasStored: boolean,
  clearSecrets: boolean,
): Record<string, string> | undefined => {
  if (clearSecrets) {
    const parsed = parseKvLines(lines);
    return Object.keys(parsed).length > 0 ? parsed : {};
  }
  if (isStoredSecretMask(lines) || (lines.trim() === "" && hasStored)) return undefined;
  const parsed = parseKvLines(lines);
  return Object.keys(parsed).length > 0 ? parsed : undefined;
};

export const McpServerFormDialog = ({
  open,
  draft,
  onOpenChange,
}: McpServerFormDialogProps): ReactNode => {
  if (!open) return null;
  return (
    <McpServerFormBody key={draft?.id ?? "create"} draft={draft} onOpenChange={onOpenChange} />
  );
};

type McpServerFormBodyProps = {
  draft?: McpServer;
  onOpenChange: (open: boolean) => void;
};

const McpServerFormBody = ({ draft, onOpenChange }: McpServerFormBodyProps): ReactNode => {
  const { t } = useTranslation();
  const save = useMcpServersStore((state) => state.save);
  const [name, setName] = useState(draft?.name ?? "");
  const [enabled, setEnabled] = useState(draft?.enabled ?? true);
  const [transport, setTransport] = useState<McpTransport>(draft?.transport ?? "stdio");
  const [command, setCommand] = useState(draft?.command ?? "");
  const [args, setArgs] = useState((draft?.args ?? []).join("\n"));
  const [cwd, setCwd] = useState(draft?.cwd ?? "");
  const [url, setUrl] = useState(draft?.url ?? "");
  const [envLines, setEnvLines] = useState(storedSecretDisplay(draft?.hasEnvSecrets));
  const [headerLines, setHeaderLines] = useState(storedSecretDisplay(draft?.hasHeaderSecrets));
  const [clearSecrets, setClearSecrets] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isEditing = Boolean(draft?.id);
  const isStdio = transport === "stdio";
  const probesTools = transport === "stdio" || transport === "http";
  const hasEnvSecrets = Boolean(draft?.hasEnvSecrets);
  const hasHeaderSecrets = Boolean(draft?.hasHeaderSecrets);
  const hasSecrets = hasEnvSecrets || hasHeaderSecrets;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!name.trim()) {
      setError(t("mcpServers.form.errors.nameRequired"));
      return;
    }
    if (isStdio && !command.trim()) {
      setError(t("mcpServers.form.errors.commandRequired"));
      return;
    }
    if (!isStdio && !url.trim()) {
      setError(t("mcpServers.form.errors.urlRequired"));
      return;
    }
    setSubmitting(true);
    setError(null);
    const env = resolveKvSecrets(envLines, hasEnvSecrets, clearSecrets);
    const headers = resolveKvSecrets(headerLines, hasHeaderSecrets, clearSecrets);
    const result = await save({
      id: draft?.id,
      name: name.trim(),
      enabled,
      transport,
      command: isStdio ? command.trim() : undefined,
      args: isStdio ? splitArgs(args) : [],
      cwd: isStdio && cwd.trim() ? cwd.trim() : undefined,
      url: !isStdio ? url.trim() : undefined,
      env,
      headers,
      clearSecrets,
    });
    setSubmitting(false);
    if (result.error) {
      setError(result.error);
      return;
    }
    if (result.server?.toolsProbeError) {
      setError(t("mcpServers.form.toolsProbeFailed", { error: result.server.toolsProbeError }));
      return;
    }
    onOpenChange(false);
  };

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      titleKey={isEditing ? "mcpServers.form.editTitle" : "mcpServers.form.createTitle"}
      size="default"
      placement="center"
      footer={
        <>
          <GlassButton
            variant="secondary"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            {t("mcpServers.form.cancel")}
          </GlassButton>
          <GlassButton
            variant="primary"
            type="submit"
            form="mcp-server-form"
            disabled={submitting || !name.trim()}
          >
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>
                  {probesTools ? t("mcpServers.form.probing") : t("mcpServers.form.saving")}
                </span>
              </>
            ) : (
              <span>{isEditing ? t("mcpServers.form.save") : t("mcpServers.form.add")}</span>
            )}
          </GlassButton>
        </>
      }
    >
      <form
        id="mcp-server-form"
        className="skill-form provider-form"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <div className="field">
          <label className="field__label" htmlFor="mcp-server-name">
            {t("mcpServers.form.name")}
          </label>
          <input
            id="mcp-server-name"
            className="input"
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoComplete="off"
            required
          />
        </div>

        <div className="field">
          <span className="field__label">{t("mcpServers.form.enabled")}</span>
          <Toggle
            checked={enabled}
            onChange={setEnabled}
            label={t("mcpServers.form.enabled")}
            showLabel={false}
          />
        </div>

        <div className="field">
          <label className="field__label" htmlFor="mcp-server-transport">
            {t("mcpServers.form.transport")}
          </label>
          <Select
            id="mcp-server-transport"
            value={transport}
            onChange={(next) => setTransport(next as McpTransport)}
            options={MCP_TRANSPORTS.map((value) => ({
              value,
              label: t(`mcpServers.transports.${value}`),
            }))}
          />
        </div>

        {isStdio ? (
          <>
            <div className="field">
              <label className="field__label" htmlFor="mcp-server-command">
                {t("mcpServers.form.command")}
              </label>
              <input
                id="mcp-server-command"
                className="input input--mono"
                type="text"
                value={command}
                onChange={(event) => setCommand(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                required
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="mcp-server-args">
                {t("mcpServers.form.args")}
              </label>
              <textarea
                id="mcp-server-args"
                className="input skill-form__textarea"
                value={args}
                onChange={(event) => setArgs(event.target.value)}
                spellCheck={false}
              />
              <span className="field__hint">{t("mcpServers.form.argsHint")}</span>
            </div>
            <div className="field">
              <label className="field__label" htmlFor="mcp-server-cwd">
                {t("mcpServers.form.cwd")}
              </label>
              <input
                id="mcp-server-cwd"
                className="input input--mono"
                type="text"
                value={cwd}
                onChange={(event) => setCwd(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </>
        ) : (
          <div className="field">
            <label className="field__label" htmlFor="mcp-server-url">
              {t("mcpServers.form.url")}
            </label>
            <input
              id="mcp-server-url"
              className="input input--mono"
              type="url"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              autoComplete="off"
              spellCheck={false}
              required
            />
          </div>
        )}

        <div className="field">
          <label className="field__label" htmlFor="mcp-server-env">
            {t("mcpServers.form.env")}
          </label>
          <textarea
            id="mcp-server-env"
            className="input skill-form__textarea"
            value={envLines}
            onFocus={() => clearStoredSecretMask(envLines, () => setEnvLines(""))}
            onChange={(event) => {
              setEnvLines(event.target.value);
              if (event.target.value.trim()) setClearSecrets(false);
            }}
            spellCheck={false}
            placeholder={t("mcpServers.form.envPlaceholder")}
          />
          <span className="field__hint">{t("mcpServers.form.envHint")}</span>
          {isEditing && hasSecrets && !clearSecrets ? (
            <button
              type="button"
              className="field__action"
              onClick={() => {
                setClearSecrets(true);
                setEnvLines("");
                setHeaderLines("");
              }}
            >
              {t("mcpServers.form.secretsRemove")}
            </button>
          ) : null}
        </div>

        {!isStdio ? (
          <div className="field">
            <label className="field__label" htmlFor="mcp-server-headers">
              {t("mcpServers.form.headers")}
            </label>
            <textarea
              id="mcp-server-headers"
              className="input skill-form__textarea"
              value={headerLines}
              onFocus={() => clearStoredSecretMask(headerLines, () => setHeaderLines(""))}
              onChange={(event) => {
                setHeaderLines(event.target.value);
                if (event.target.value.trim()) setClearSecrets(false);
              }}
              spellCheck={false}
              placeholder={t("mcpServers.form.headersPlaceholder")}
            />
            <span className="field__hint">{t("mcpServers.form.headersHint")}</span>
          </div>
        ) : null}

        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}
      </form>
    </Dialog>
  );
};
