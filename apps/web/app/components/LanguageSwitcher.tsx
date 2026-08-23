"use client";

import { useTranslation, type Locale, SUPPORTED_LOCALES } from "../../lib/i18n";

export function LanguageSwitcher() {
  const { locale, setLocale, t } = useTranslation();
  return (
    <div className="lang-switcher" role="group" aria-label="Language">
      {SUPPORTED_LOCALES.map((l: Locale) => (
        <button
          key={l}
          type="button"
          onClick={() => setLocale(l)}
          disabled={locale === l}
          aria-pressed={locale === l}
          aria-label={t.languages[l]}
        >
          {l.toUpperCase()}
        </button>
      ))}
    </div>
  );
}