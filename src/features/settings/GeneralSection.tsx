import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { SUPPORTED_LANGUAGES, type AppLanguage, type AppTheme } from "@/types/settings";
import { useSettingsStore } from "@/lib/settings";
import { Toggle } from "@/components/Toggle";

const themeOptions: readonly AppTheme[] = ["dark", "light"];

export const GeneralSection = (): ReactNode => {
  const { t } = useTranslation();
  const language = useSettingsStore((state) => state.language);
  const theme = useSettingsStore((state) => state.theme);
  const minimizeToTray = useSettingsStore((state) => state.minimizeToTray);
  const translucencyEnabled = useSettingsStore((state) => state.translucencyEnabled);
  const setLanguage = useSettingsStore((state) => state.setLanguage);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const setMinimizeToTray = useSettingsStore((state) => state.setMinimizeToTray);
  const setTranslucencyEnabled = useSettingsStore((state) => state.setTranslucencyEnabled);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("settings.sections.general")}</h2>
      </header>

      <div className="field">
        <label className="field__label" htmlFor="language-select">
          {t("settings.language.label")}
        </label>
        <select
          id="language-select"
          className="select"
          value={language}
          onChange={(event) => setLanguage(event.target.value as AppLanguage)}
        >
          {SUPPORTED_LANGUAGES.map((code) => (
            <option key={code} value={code}>
              {t(`settings.language.options.${code}`)}
            </option>
          ))}
        </select>
        <span className="field__hint">{t("settings.language.description")}</span>
      </div>

      <div className="field">
        <label className="field__label" htmlFor="theme-select">
          {t("settings.theme.label")}
        </label>
        <select
          id="theme-select"
          className="select"
          value={theme}
          onChange={(event) => setTheme(event.target.value as AppTheme)}
        >
          {themeOptions.map((value) => (
            <option key={value} value={value}>
              {t(`settings.theme.options.${value}`)}
            </option>
          ))}
        </select>
        <span className="field__hint">{t("settings.theme.description")}</span>
      </div>

      <Toggle
        checked={minimizeToTray}
        onChange={setMinimizeToTray}
        label={t("settings.tray.label")}
        description={t("settings.tray.description")}
      />

      <Toggle
        checked={!translucencyEnabled}
        onChange={(next) => setTranslucencyEnabled(!next)}
        label={t("settings.translucency.label")}
        description={t("settings.translucency.description")}
      />
    </section>
  );
};
