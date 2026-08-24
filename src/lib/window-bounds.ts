import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import { useSettingsStore } from "@/lib/settings";
import type { WindowBounds } from "@/types/settings";

const SAVE_DELAY_MS = 250;

export const useWindowBoundsSync = (): void => {
  const hydrated = useSettingsStore((state) => state.hydrated);
  const remember = useSettingsStore((state) => state.rememberWindowSize);
  const setWindowBounds = useSettingsStore((state) => state.setWindowBounds);
  const applyingRef = useRef(false);

  useEffect(() => {
    if (!hydrated || !isTauri()) return;
    const { rememberWindowSize, windowBounds } = useSettingsStore.getState();
    if (!rememberWindowSize) return;

    applyingRef.current = true;
    void invoke("window_apply_bounds", { bounds: windowBounds })
      .catch((error: unknown) => {
        console.warn("window_apply_bounds failed", error);
      })
      .finally(() => {
        window.setTimeout(() => {
          applyingRef.current = false;
        }, 80);
      });
  }, [hydrated]);

  useEffect(() => {
    if (!hydrated || !isTauri() || !remember) return;

    let timer = 0;
    let active = true;

    const save = (): void => {
      if (applyingRef.current) return;
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

    window.addEventListener("resize", save);
    return () => {
      active = false;
      window.clearTimeout(timer);
      window.removeEventListener("resize", save);
    };
  }, [hydrated, remember, setWindowBounds]);
};
