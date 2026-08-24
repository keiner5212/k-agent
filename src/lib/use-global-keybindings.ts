import { useEffect } from "react";
import { matchesChordString } from "./keybindings";
import { useSettingsStore } from "./settings";

type KeybindingsHook = {
  register: (
    action: keyof ReturnType<typeof useSettingsStore.getState>["keybindings"],
    handler: () => void,
  ) => void;
};

export const useGlobalKeybindings = (
  onAction: (action: keyof ReturnType<typeof useSettingsStore.getState>["keybindings"]) => void,
): KeybindingsHook => {
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
          onAction(action as keyof typeof keybindings);
          return;
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [keybindings, onAction]);

  return {
    register: (action, handler) => {
      if (action === "settings.open") onAction(action);
      void handler;
    },
  };
};
