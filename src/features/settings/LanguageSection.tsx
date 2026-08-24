import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { SUPPORTED_LANGUAGES, type AppLanguage } from "@/types/settings";
import { settingsSelector, useSettingsStore } from "@/lib/settings";

export const LanguageSection = (): ReactNode => {
  const { t } = useTranslation();
  const language = useSettingsStore(settingsSelector.language);
  const setLanguage = useSettingsStore((state) => state.setLanguage);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("settings.sections.language")}</h2>
        <p className="section__description">{t("settings.language.description")}</p>
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
      </div>
    </section>
  );
};
