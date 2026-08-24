import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";

const EDGES = [
  "north",
  "south",
  "east",
  "west",
  "north-east",
  "north-west",
  "south-east",
  "south-west",
] as const;

type ResizeEdge = (typeof EDGES)[number];

export const WindowResizeFrame = (): ReactNode => {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;

    const query = async (): Promise<void> => {
      try {
        const value = await invoke<boolean>("window_is_maximized");
        if (active) setMaximized(value);
      } catch {
        if (active) setMaximized(false);
      }
    };

    void query();
    const onResize = (): void => {
      void query();
    };
    window.addEventListener("resize", onResize);
    return () => {
      active = false;
      window.removeEventListener("resize", onResize);
    };
  }, []);

  if (!isTauri() || maximized) return null;

  const startResize = (edge: ResizeEdge): void => {
    void invoke("window_start_resize", { edge }).catch((error: unknown) => {
      console.warn("window_start_resize failed", error);
    });
  };

  return (
    <div className="window-resize" aria-hidden="true">
      {EDGES.map((edge) => (
        <div
          key={edge}
          className={`window-resize__edge window-resize__edge--${edge}`}
          onMouseDown={(event) => {
            event.preventDefault();
            startResize(edge);
          }}
        />
      ))}
    </div>
  );
};
