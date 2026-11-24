import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { SquareTerminal } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { isTauri } from "@/lib/platform";
import { useSessionsStore } from "@/lib/sessions";

type BackgroundProcess = {
  pid: number;
  command: string;
  running: boolean;
};

export const BackgroundDock = (): ReactNode => {
  const { t } = useTranslation();
  const sessionId = useSessionsStore((state) => state.activeSessionId);
  const turnLive = useSessionsStore(
    (state) => sessionId !== null && state.sendingSessionIds.includes(sessionId),
  );
  const [processes, setProcesses] = useState<BackgroundProcess[]>([]);
  const [openPid, setOpenPid] = useState<number | null>(null);
  const [log, setLog] = useState("");
  const watch = `${sessionId ?? ""}:${turnLive ? "1" : "0"}`;
  const [tracked, setTracked] = useState(watch);
  if (tracked !== watch) {
    setTracked(watch);
    setProcesses([]);
    setOpenPid(null);
  }

  useEffect(() => {
    if (!sessionId || !isTauri() || !turnLive) return;
    let alive = true;
    const load = (): void => {
      void invoke<BackgroundProcess[]>("list_background_processes", { sessionId })
        .then((items) => {
          if (!alive) return;
          const running = items.filter((item) => item.running);
          setProcesses(running);
          setOpenPid((current) =>
            current !== null && running.some((item) => item.pid === current) ? current : null,
          );
        })
        .catch(() => {
          if (!alive) return;
          setProcesses([]);
          setOpenPid(null);
        });
    };
    load();
    const timer = window.setInterval(load, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [sessionId, turnLive]);

  useEffect(() => {
    if (!sessionId || openPid === null || !isTauri()) return;
    let alive = true;
    const load = (): void => {
      void invoke<string>("read_background_log", { sessionId, pid: openPid })
        .then((text) => {
          if (alive) setLog(text);
        })
        .catch(() => {
          if (alive) setLog("");
        });
    };
    load();
    const timer = window.setInterval(load, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [openPid, sessionId]);

  const visible = !sessionId || !isTauri() || !turnLive || tracked !== watch ? [] : processes;
  if (visible.length === 0) return null;
  const open = visible.find((item) => item.pid === openPid) ?? null;

  return (
    <>
      {visible.map((item) => (
        <button
          key={item.pid}
          type="button"
          className="context-strip__chip context-strip__chip--button"
          title={item.command}
          onClick={() => setOpenPid(item.pid)}
        >
          <SquareTerminal size={12} strokeWidth={1.5} />
          <span className="context-strip__chip-value">
            {item.command} {item.pid}
          </span>
        </button>
      ))}
      <Dialog
        open={open !== null}
        onOpenChange={(next) => {
          if (!next) setOpenPid(null);
        }}
        titleKey="chat.tools.backgroundTitle"
        size="wide"
        placement="center"
        footer={
          open ? (
            <GlassButton
              type="button"
              onClick={() => {
                const pid = open.pid;
                void invoke("stop_background_process", { sessionId, pid }).finally(() => {
                  setOpenPid(null);
                  setProcesses((items) => items.filter((item) => item.pid !== pid));
                });
              }}
            >
              {t("chat.tools.backgroundStop")}
            </GlassButton>
          ) : null
        }
      >
        <pre className="background-log">{log || t("chat.tools.backgroundEmpty")}</pre>
      </Dialog>
    </>
  );
};
