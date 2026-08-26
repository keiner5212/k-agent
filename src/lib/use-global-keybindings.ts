import { useEffect } from "react";
import { closeAllDialogs, closeTopDialog, hasOpenDialogs } from "./dialog-stack";
import { isEditableTarget, matchesChordString } from "./keybindings";
import { INTERRUPT_ARM_MS, useSessionsStore } from "./sessions";
import { useSettingsStore } from "./settings";
import type { KeybindingAction } from "@/types/settings";

const isKeybindingCaptureActive = (): boolean =>
  Boolean(document.querySelector(".kbd-capture[data-recording='true']"));

const isVisible = (el: HTMLElement): boolean =>
  el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden";

const topVisibleSearchInput = (): HTMLInputElement | null => {
  const dialogs = document.querySelectorAll(".dialog-root");
  const topDialog = dialogs.length > 0 ? dialogs[dialogs.length - 1] : null;
  const scope: ParentNode = topDialog ?? document;
  const inputs = [...scope.querySelectorAll<HTMLInputElement>('input[type="search"]')];
  for (let index = inputs.length - 1; index >= 0; index -= 1) {
    const input = inputs[index];
    if (input && !input.disabled && isVisible(input)) return input;
  }
  return null;
};

const focusVisibleSearch = (): boolean => {
  const search = topVisibleSearchInput();
  if (!search) return false;
  search.focus();
  search.select();
  return true;
};

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
        const sessions = useSessionsStore.getState();
        if (sessions.sending || sessions.shellRunning) {
          event.preventDefault();
          event.stopPropagation();
          const now = Date.now();
          const armed =
            sessions.interruptArmedAt !== null &&
            now - sessions.interruptArmedAt < INTERRUPT_ARM_MS;
          if (armed) {
            void sessions.interruptActiveTask();
          } else {
            sessions.armInterrupt();
          }
          return;
        }
      }

      const hasModifier = event.ctrlKey || event.metaKey || event.altKey;
      if (isEditableTarget(event.target) && !hasModifier && event.key !== "Escape") return;

      for (const [action, chord] of Object.entries(keybindings)) {
        if (!matchesChordString(event, chord)) continue;
        if (action === "search.focus") {
          if (!focusVisibleSearch()) continue;
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        if (action === "chat.modeToggle" || action === "chat.agentCycle") continue;
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
