import { useEffect, useRef, type RefObject } from "react";
import { applyComposerAgentCycle, matchesAgentCycle } from "@/lib/composer-agents";
import { hasOpenDialogs } from "@/lib/dialog-stack";
import { isEditableTarget, matchesChordString } from "@/lib/keybindings";
import { useComposerStore } from "@/lib/composer";
import { useSettingsStore, type SettingsStore } from "@/lib/settings";

const isActivateTarget = (target: EventTarget | null): boolean => {
  if (!(target instanceof Element)) return false;
  return (
    target.closest(
      "button, a, summary, [role='button'], [role='option'], [role='menuitem'], [role='listbox']",
    ) !== null
  );
};

const overlayBlocksComposerFocus = (): boolean => {
  if (hasOpenDialogs()) return true;
  if (document.querySelector(".kbd-capture[data-recording='true']")) return true;
  return Boolean(document.querySelector(".select-menu, .model-selector__popover"));
};

const insertAtCursor = (
  field: HTMLTextAreaElement,
  token: string,
  onInsert: (next: string) => void,
): void => {
  const start = field.selectionStart ?? field.value.length;
  const end = field.selectionEnd ?? start;
  const next = `${field.value.slice(0, start)}${token}${field.value.slice(end)}`;
  onInsert(next);
  requestAnimationFrame(() => {
    const pos = start + token.length;
    field.setSelectionRange(pos, pos);
    field.dispatchEvent(new Event("select"));
  });
};

export const useComposerFocusKeys = (
  textareaRef: RefObject<HTMLTextAreaElement | null>,
  onInsert: (next: string) => void,
): void => {
  const onInsertRef = useRef(onInsert);
  const modeToggleBinding = useSettingsStore(
    (state: SettingsStore) => state.keybindings["chat.modeToggle"],
  );
  const agentCycleBinding = useSettingsStore(
    (state: SettingsStore) => state.keybindings["chat.agentCycle"],
  );

  useEffect(() => {
    onInsertRef.current = onInsert;
  }, [onInsert]);

  useEffect(() => {
    const handler = (event: KeyboardEvent): void => {
      if (event.defaultPrevented) return;
      if (overlayBlocksComposerFocus()) return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;

      const field = textareaRef.current;
      if (!field) return;

      if (matchesChordString(event, modeToggleBinding)) {
        if (isEditableTarget(event.target) && event.target !== field) return;
        event.preventDefault();
        field.focus();
        if (useComposerStore.getState().value.trim().length === 0) {
          useComposerStore.getState().toggleMode();
        }
        return;
      }

      if (matchesAgentCycle(event, agentCycleBinding)) {
        if (event.target === field) return;
        if (isEditableTarget(event.target)) return;
        if (applyComposerAgentCycle(1)) {
          event.preventDefault();
          field.focus();
        }
        return;
      }

      if (event.target === field) return;
      if (isEditableTarget(event.target)) return;

      if (event.key === "Enter") {
        if (event.shiftKey) return;
        if (isActivateTarget(event.target)) return;
        event.preventDefault();
        field.focus();
        return;
      }

      if (event.key === "@" || event.key === "/") {
        event.preventDefault();
        field.focus();
        insertAtCursor(field, event.key, onInsertRef.current);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [agentCycleBinding, modeToggleBinding, textareaRef]);
};
