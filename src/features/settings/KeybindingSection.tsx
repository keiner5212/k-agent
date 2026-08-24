import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { chordToString, eventToChord, parseChord } from "@/lib/keybindings";
import { useSettingsStore } from "@/lib/settings";
import type { KeybindingAction } from "@/types/settings";

const actionOrder: readonly KeybindingAction[] = ["settings.open", "settings.close"];

const KeybindingRow = ({ action }: { action: KeybindingAction }): ReactNode => {
  const { t } = useTranslation();
  const chord = useSettingsStore((state) => state.keybindings[action]);
  const setKeybinding = useSettingsStore((state) => state.setKeybinding);
  const [recording, setRecording] = useState(false);
  const [draft, setDraft] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;

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

      const parsed = parseChord(event);
      if (!parsed) {
        setDraft(chordToString(eventToChord(event)));
        return;
      }
      setKeybinding(action, chordToString(parsed));
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
    <li className="kbd-row">
      <span className="kbd-row__action">{t(`settings.keybindings.actions.${action}`)}</span>
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
    </li>
  );
};

export const KeybindingSection = (): ReactNode => {
  const { t } = useTranslation();
  const resetKeybindings = useSettingsStore((state) => state.resetKeybindings);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("settings.sections.keybindings")}</h2>
        <p className="section__description">{t("settings.keybindings.description")}</p>
      </header>
      <ul className="kbd-list">
        {actionOrder.map((action) => (
          <KeybindingRow key={action} action={action} />
        ))}
      </ul>
      <div>
        <button type="button" className="btn btn--ghost" onClick={resetKeybindings}>
          {t("settings.keybindings.reset")}
        </button>
      </div>
    </section>
  );
};
