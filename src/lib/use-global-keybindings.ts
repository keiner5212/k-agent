import { useEffect } from "react";
import { matchesChordString } from "./keybindings";
import { useSettingsStore } from "./settings";
import type { KeybindingAction } from "@/types/settings";

const isEditableTarget = (target: EventTarget | null): boolean =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLElement && target.isContentEditable);

export const useGlobalKeybindings = (onAction: (action: KeybindingAction) => void): void => {
  const keybindings = useSettingsStore((state) => state.keybindings);

  useEffect(() => {
    const handler = (event: KeyboardEvent): void => {
      if (event.defaultPrevented) return;
      if (document.querySelector(".kbd-capture[data-recording='true']")) return;

      const hasModifier = event.ctrlKey || event.metaKey || event.altKey;
      if (isEditableTarget(event.target) && !hasModifier && event.key !== "Escape") return;

      for (const [action, chord] of Object.entries(keybindings)) {
        if (!matchesChordString(event, chord)) continue;
        event.preventDefault();
        event.stopPropagation();
        onAction(action as KeybindingAction);
        return;
      }
    };

    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [keybindings, onAction]);
};
