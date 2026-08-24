import { useEffect } from "react";
import { matchesChordString } from "./keybindings";
import { useSettingsStore } from "./settings";
import type { KeybindingAction } from "@/types/settings";

export const useGlobalKeybindings = (onAction: (action: KeybindingAction) => void): void => {
  const keybindings = useSettingsStore((state) => state.keybindings);

  useEffect(() => {
    const handler = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      const inEditableField =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target?.isContentEditable ?? false);

      if (inEditableField && !(event.ctrlKey || event.metaKey)) return;

      for (const [action, chord] of Object.entries(keybindings)) {
        if (matchesChordString(event, chord)) {
          event.preventDefault();
          onAction(action as KeybindingAction);
          return;
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [keybindings, onAction]);
};
