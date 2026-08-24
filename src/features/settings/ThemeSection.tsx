import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { AppTheme } from "@/types/settings";
import { settingsSelector, useSettingsStore } from "@/lib/settings";

const themeOptions: readonly AppTheme[] = ["dark", "light"];

export const ThemeSection = (): ReactNode => {
  const { t } = useTranslation();
  const theme = useSettingsStore(settingsSelector.theme);
  const setTheme = useSettingsStore((state) => state.setTheme);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("settings.sections.theme")}</h2>
        <p className="section__description">{t("settings.theme.description")}</p>
      </header>
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
      </div>
    </section>
  );
};
