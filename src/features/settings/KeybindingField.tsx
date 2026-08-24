import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/lib/settings";
import type { KeybindingAction } from "@/types/settings";

type KeybindingFieldProps = {
  action: KeybindingAction;
};

export const KeybindingField = ({ action }: KeybindingFieldProps): ReactNode => {
  const { t } = useTranslation();
  const chord = useSettingsStore((state) => state.keybindings[action]);
  const setKeybinding = useSettingsStore((state) => state.setKeybinding);
  const [recording, setRecording] = useState(false);
  const [draft, setDraft] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;

    const isModifierKey = (key: string): boolean =>
      key === "Control" || key === "Shift" || key === "Alt" || key === "Meta";

    const handleKey = (event: KeyboardEvent): void => {
      event.preventDefault();
      event.stopPropagation();

      if (
        event.key === "Escape" &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.altKey &&
        !event.shiftKey
      ) {
        setRecording(false);
        setDraft(null);
        return;
      }

      const parts: string[] = [];
      if (event.metaKey) parts.push("Meta");
      if (event.ctrlKey) parts.push("Ctrl");
      if (event.altKey) parts.push("Alt");
      if (event.shiftKey) parts.push("Shift");

      if (isModifierKey(event.key)) {
        setDraft(parts.length > 0 ? `${parts.join("+")}+` : "");
        return;
      }

      const key = event.key === " " ? "Space" : event.key;
      const formatted = key.length === 1 ? key.toUpperCase() : key;
      parts.push(formatted);
      setKeybinding(action, parts.join("+"));
      setRecording(false);
      setDraft(null);
    };

    window.addEventListener("keydown", handleKey, { capture: true });
    return () => window.removeEventListener("keydown", handleKey, { capture: true });
  }, [recording, action, setKeybinding]);

  const display = recording
    ? (draft ?? t("settings.keybindings.record"))
    : chord || t("settings.keybindings.unset");

  return (
    <button
      type="button"
      className="kbd-capture"
      data-recording={recording}
      data-empty={!recording && !chord}
      onClick={() => {
        setRecording((current) => !current);
        setDraft(null);
      }}
    >
      {display}
    </button>
  );
};
