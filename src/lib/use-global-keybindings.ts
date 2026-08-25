import { useEffect } from "react";
import { closeAllDialogs, closeTopDialog, hasOpenDialogs } from "./dialog-stack";
import { matchesChordString } from "./keybindings";
import { useSettingsStore } from "./settings";
import type { KeybindingAction } from "@/types/settings";

const isEditableTarget = (target: EventTarget | null): boolean =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLElement && target.isContentEditable);

const isKeybindingCaptureActive = (): boolean =>
  Boolean(document.querySelector(".kbd-capture[data-recording='true']"));

export const useGlobalKeybindings = (onAction: (action: KeybindingAction) => void): void => {
  const keybindings = useSettingsStore((state) => state.keybindings);

  useEffect(() => {
    const handler = (event: KeyboardEvent): void => {
      if (event.defaultPrevented) return;
      if (isKeybindingCaptureActive()) return;

      if (event.key === "Escape") {
        if (event.ctrlKey) {
          if (hasOpenDialogs()) {
            event.preventDefault();
            event.stopPropagation();
            closeAllDialogs();
          }
          return;
        }
        if (hasOpenDialogs()) {
          event.preventDefault();
          event.stopPropagation();
          closeTopDialog();
          return;
        }
      }

      const hasModifier = event.ctrlKey || event.metaKey || event.altKey;
      if (isEditableTarget(event.target) && !hasModifier && event.key !== "Escape") return;

      for (const [action, chord] of Object.entries(keybindings)) {
        if (!matchesChordString(event, chord)) continue;
        const inUndoableText =
          event.target instanceof HTMLTextAreaElement &&
          (event.target.closest(".line-editor") !== null ||
            event.target.classList.contains("chat-composer__input"));
        if ((action === "editor.undo" || action === "editor.redo") && inUndoableText) {
          return;
        }
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
