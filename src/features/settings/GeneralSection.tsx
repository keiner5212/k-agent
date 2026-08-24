import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import {
  SUPPORTED_LANGUAGES,
  TEXT_SCALE_OPTIONS,
  type AppLanguage,
  type AppTheme,
  type TextScale,
} from "@/types/settings";
import { useSettingsStore } from "@/lib/settings";

const themeOptions: readonly AppTheme[] = ["dark", "light"];

const textScaleLabel = (scale: TextScale): string => `${Math.round(scale * 100)}%`;

export const GeneralSection = (): ReactNode => {
  const { t } = useTranslation();
  const language = useSettingsStore((state) => state.language);
  const theme = useSettingsStore((state) => state.theme);
  const minimizeToTray = useSettingsStore((state) => state.minimizeToTray);
  const translucencyEnabled = useSettingsStore((state) => state.translucencyEnabled);
  const animationsEnabled = useSettingsStore((state) => state.animationsEnabled);
  const textScale = useSettingsStore((state) => state.textScale);
  const setLanguage = useSettingsStore((state) => state.setLanguage);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const setMinimizeToTray = useSettingsStore((state) => state.setMinimizeToTray);
  const setTranslucencyEnabled = useSettingsStore((state) => state.setTranslucencyEnabled);
  const setAnimationsEnabled = useSettingsStore((state) => state.setAnimationsEnabled);
  const setTextScale = useSettingsStore((state) => state.setTextScale);

  return (
    <section className="section">
      <header>
        <h2 className="section__heading">{t("settings.sections.general")}</h2>
      </header>

      <div className="field">
        <label className="field__label" htmlFor="language-select">
          {t("settings.language.label")}
        </label>
        <Select
          id="language-select"
          value={language}
          onChange={(next) => setLanguage(next as AppLanguage)}
          options={SUPPORTED_LANGUAGES.map((code) => ({
            value: code,
            label: t(`settings.language.options.${code}`),
          }))}
        />
      </div>

      <div className="field">
        <label className="field__label" htmlFor="theme-select">
          {t("settings.theme.label")}
        </label>
        <Select
          id="theme-select"
          value={theme}
          onChange={(next) => setTheme(next as AppTheme)}
          options={themeOptions.map((value) => ({
            value,
            label: t(`settings.theme.options.${value}`),
          }))}
        />
      </div>

      <div className="field">
        <label className="field__label" htmlFor="text-scale-select">
          {t("settings.textScale.label")}
        </label>
        <Select
          id="text-scale-select"
          value={String(textScale)}
          onChange={(next) => setTextScale(Number(next) as TextScale)}
          options={TEXT_SCALE_OPTIONS.map((scale) => ({
            value: String(scale),
            label: textScaleLabel(scale),
          }))}
        />
        <span className="field__hint">{t("settings.textScale.description")}</span>
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

      <Toggle
        checked={!animationsEnabled}
        onChange={(next) => setAnimationsEnabled(!next)}
        label={t("settings.animations.label")}
        description={t("settings.animations.description")}
      />
    </section>
  );
};
