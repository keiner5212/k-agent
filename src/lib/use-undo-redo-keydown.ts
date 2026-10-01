import { useEffect, type RefObject } from "react";
import { matchesChordString } from "@/lib/keybindings";
import type { Keybindings } from "@/types/settings";

export const useUndoRedoKeydown = (
  ref: RefObject<HTMLTextAreaElement | null>,
  keybindings: Keybindings,
  undo: () => void,
  redo: () => void,
  enabled = true,
): void => {
  useEffect(() => {
    const textarea = ref.current;
    if (!textarea || !enabled) return;

    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.defaultPrevented) return;
      if (matchesChordString(event, keybindings["editor.undo"])) {
        event.preventDefault();
        undo();
        return;
      }
      if (matchesChordString(event, keybindings["editor.redo"])) {
        event.preventDefault();
        redo();
      }
    };

    textarea.addEventListener("keydown", onKeyDown);
    return () => textarea.removeEventListener("keydown", onKeyDown);
  }, [ref, keybindings, undo, redo, enabled]);
};
