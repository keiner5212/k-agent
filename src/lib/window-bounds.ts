import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import { useSettingsStore } from "@/lib/settings";
import type { WindowBounds } from "@/types/settings";

const SAVE_DELAY_MS = 250;

const sizeClose = (a: WindowBounds, b: WindowBounds): boolean =>
  Math.abs(a.width - b.width) < 2 && Math.abs(a.height - b.height) < 2;

const applySavedBounds = async (): Promise<void> => {
  const { windowBounds } = useSettingsStore.getState();
  await invoke("window_apply_bounds", { bounds: windowBounds });
  if (windowBounds.maximized) return;
  const now = await invoke<WindowBounds>("window_get_bounds");
  if (!now.maximized && !sizeClose(now, windowBounds)) {
    await invoke("window_apply_bounds", { bounds: windowBounds });
  }
};

export const useWindowBoundsSync = (): void => {
  const hydrated = useSettingsStore((state) => state.hydrated);
  const remember = useSettingsStore((state) => state.rememberWindowSize);
  const setWindowBounds = useSettingsStore((state) => state.setWindowBounds);

  useEffect(() => {
    if (!hydrated || !isTauri() || !remember) return;

    let active = true;
    let timer = 0;
    let applying = true;

    const save = (): void => {
      if (applying) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void invoke<WindowBounds>("window_get_bounds")
          .then((next) => {
            if (active) setWindowBounds(next);
          })
          .catch((error: unknown) => {
            console.warn("window_get_bounds failed", error);
          });
      }, SAVE_DELAY_MS);
    };

    void applySavedBounds()
      .catch((error: unknown) => {
        console.warn("window_apply_bounds failed", error);
      })
      .finally(() => {
        applying = false;
      });

    window.addEventListener("resize", save);
    return () => {
      active = false;
      applying = true;
      window.clearTimeout(timer);
      window.removeEventListener("resize", save);
    };
  }, [hydrated, remember, setWindowBounds]);
};
