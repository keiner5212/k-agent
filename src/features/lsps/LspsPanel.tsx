import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { highlightMatch } from "@/lib/highlight";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import {
  runInstallLanguageServerJob,
  runListLanguageServersJob,
  runUninstallLanguageServerJob,
} from "@/lib/jobs";
import type { LanguageServerRow, LspInstallProgress } from "@/types/language-servers";
import type { SettingItem as SettingItemDef } from "@/features/settings/registry";
import { SettingItem } from "@/features/settings/SettingItem";

type LspsPanelProps = {
  items: SettingItemDef[];
  query: string;
};

const rowMatches = (row: LanguageServerRow, query: string): boolean => {
  if (query.length === 0) return true;
  const haystack = [
    row.id,
    row.name,
    row.command,
    ...(row.languageIds ?? []),
    ...(row.extensions ?? []),
    ...(row.requires ?? []),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(query.toLowerCase());
};

export const LspsPanel = ({ items, query }: LspsPanelProps): ReactNode => {
  const { t } = useTranslation();
  const [rows, setRows] = useState<LanguageServerRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | undefined>();
  const [busyId, setBusyId] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<LspInstallProgress | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const load = useCallback(async (): Promise<void> => {
    setLoading(true);
    setError(undefined);
    if (!isTauri()) {
      setRows([]);
      setLoading(false);
      setError(DESKTOP_REQUIRED);
      return;
    }
    try {
      const next = await runListLanguageServersJob();
      setRows(next);
    } catch (caught: unknown) {
      setError(ipcErrorMessage(caught));
    } finally {
      setLoading(false);
    }
  }, []);

  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen<LspInstallProgress>("lsp-install-progress", (event) => {
      setInstallProgress(event.payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const visible = useMemo(() => rows.filter((row) => rowMatches(row, query)), [rows, query]);

  useEffect(() => {
    const node = wrapRef.current;
    if (!node) return;
    const onWheel = (event: WheelEvent): void => {
      const max = node.scrollHeight - node.clientHeight;
      if (max <= 0) return;
      const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? node.clientHeight : 1;
      const next = Math.min(max, Math.max(0, node.scrollTop + event.deltaY * unit));
      if (next === node.scrollTop) return;
      event.preventDefault();
      node.scrollTop = next;
    };
    node.addEventListener("wheel", onWheel, { passive: false });
    return () => node.removeEventListener("wheel", onWheel);
  }, [loading, visible.length]);

  const install = async (id: string): Promise<void> => {
    setBusyId(id);
    setInstallProgress(null);
    setRowError((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    try {
      const updated = await runInstallLanguageServerJob(id);
      const refreshed = await runListLanguageServersJob();
      setRows(refreshed.length > 0 ? refreshed : [updated]);
    } catch (caught: unknown) {
      setRowError((current) => ({ ...current, [id]: ipcErrorMessage(caught) }));
    } finally {
      setBusyId(null);
      setInstallProgress(null);
    }
  };

  const uninstall = async (id: string): Promise<void> => {
    setBusyId(id);
    setInstallProgress(null);
    setRowError((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    try {
      const updated = await runUninstallLanguageServerJob(id);
      const refreshed = await runListLanguageServersJob();
      setRows(refreshed.length > 0 ? refreshed : [updated]);
    } catch (caught: unknown) {
      setRowError((current) => ({ ...current, [id]: ipcErrorMessage(caught) }));
    } finally {
      setBusyId(null);
      setInstallProgress(null);
    }
  };

  return (
    <section className="lsps-panel">
      <div className="settings-list">
        {items.map((item) => (
          <SettingItem key={item.id} item={item} query={query} />
        ))}
      </div>

      <header className="lsps-panel__head">
        <h3 className="section__heading">{highlightMatch(t("lsps.title"), query)}</h3>
        <p className="section__description">{highlightMatch(t("lsps.description"), query)}</p>
        <IconButton
          className="lsps-panel__refresh"
          label={t("lsps.actions.refresh")}
          onClick={() => void load()}
          disabled={loading || busyId !== null}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}

      {visible.length === 0 && !loading ? (
        <p className="lsps-panel__empty">{t("lsps.empty")}</p>
      ) : (
        <div className="lsp-table-wrap" ref={wrapRef}>
          <table className="lsp-table">
            <thead>
              <tr>
                <th>{t("lsps.table.server")}</th>
                <th>{t("lsps.table.languages")}</th>
                <th>{t("lsps.table.requires")}</th>
                <th>{t("lsps.table.status")}</th>
                <th>
                  <span className="visually-hidden">{t("lsps.table.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {visible.map((row) => {
                const args = row.args ?? [];
                const missing = row.missingRequires ?? [];
                const needsTools = missing.length > 0;
                const busy = busyId === row.id;
                const progress = busy && installProgress?.id === row.id ? installProgress : null;
                const canInstall = !row.installed && !needsTools && busyId === null;
                const canUninstall = row.installed && busyId === null;
                const statusKey = row.installed
                  ? "installed"
                  : needsTools
                    ? "missingRequires"
                    : "notInstalled";
                const statusState = row.installed ? "installed" : needsTools ? "missing" : "absent";
                return (
                  <tr key={row.id}>
                    <td className="lsp-table__server">
                      <span className="lsp-table__name">{highlightMatch(row.name, query)}</span>
                      <span className="lsp-table__command">
                        {row.command}
                        {args.length > 0 ? ` ${args.join(" ")}` : ""}
                      </span>
                    </td>
                    <td className="lsp-table__langs">{(row.languageIds ?? []).join(", ")}</td>
                    <td className="lsp-table__requires">{(row.requires ?? []).join(", ")}</td>
                    <td className="lsp-table__status" data-state={statusState}>
                      {progress ? (
                        <div className="lsp-table__progress">
                          <span className="lsp-table__progress-label">
                            {progress.phase === "installing"
                              ? t("lsps.progress.installing", {
                                  tool: progress.detail ?? t("lsps.actions.installing"),
                                })
                              : t(`lsps.progress.${progress.phase}`)}
                            {progress.percent != null ? ` ${progress.percent}%` : ""}
                          </span>
                          {progress.percent != null ? (
                            <div className="lsp-table__progress-track" aria-hidden="true">
                              <div
                                className="lsp-table__progress-bar"
                                style={{ width: `${progress.percent}%` }}
                              />
                            </div>
                          ) : null}
                        </div>
                      ) : statusKey === "missingRequires" ? (
                        t("lsps.status.missingRequires", { tools: missing.join(", ") })
                      ) : (
                        t(`lsps.status.${statusKey}`)
                      )}
                      {rowError[row.id] ? (
                        <span className="lsp-table__error">{rowError[row.id]}</span>
                      ) : null}
                    </td>
                    <td className="lsp-table__actions">
                      {row.installed ? (
                        <GlassButton
                          variant="danger"
                          disabled={!canUninstall || busy}
                          onClick={() => void uninstall(row.id)}
                        >
                          {busy ? t("lsps.actions.uninstalling") : t("lsps.actions.uninstall")}
                        </GlassButton>
                      ) : (
                        <GlassButton
                          variant="primary"
                          disabled={!canInstall || busy}
                          title={
                            needsTools
                              ? t("lsps.status.missingRequires", { tools: missing.join(", ") })
                              : undefined
                          }
                          onClick={() => void install(row.id)}
                        >
                          {busy ? t("lsps.actions.installing") : t("lsps.actions.install")}
                        </GlassButton>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
};
