import { Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { isMac, isTauri } from "@/lib/platform";

const useMaximized = (): boolean => {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;

    let active = true;
    const query = async (): Promise<void> => {
      try {
        const value = await invoke<boolean>("window_is_maximized");
        if (active) setMaximized(value);
      } catch (error) {
        console.warn("window_is_maximized failed", error);
        if (active) setMaximized(false);
      }
    };

    void query();
    const id = window.setInterval(() => void query(), 400);

    const onResize = (): void => {
      void query();
    };
    window.addEventListener("resize", onResize);

    return () => {
      active = false;
      window.clearInterval(id);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return maximized;
};

export const WindowControls = (): ReactNode => {
  const { t } = useTranslation();
  const maximized = useMaximized();

  if (isMac()) return null;

  const handle = (command: string): void => {
    void invoke(command).catch((error: unknown) => {
      console.error(`window command ${command} failed`, error);
    });
  };

  return (
    <div className="window-controls">
      <button
        type="button"
        className="window-controls__btn"
        onClick={() => handle("window_minimize")}
        aria-label={t("window.minimize")}
        title={t("window.minimize")}
      >
        <Minus size={12} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        className="window-controls__btn"
        onClick={() => handle("window_toggle_maximize")}
        aria-label={maximized ? t("window.unmaximize") : t("window.maximize")}
        title={maximized ? t("window.unmaximize") : t("window.maximize")}
      >
        <Square size={11} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        className="window-controls__btn window-controls__btn--close"
        onClick={() => handle("window_close")}
        aria-label={t("window.close")}
        title={t("window.close")}
      >
        <X size={12} strokeWidth={1.5} />
      </button>
    </div>
  );
};
