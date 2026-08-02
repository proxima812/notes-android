import { createContext, useCallback, useContext, useMemo, useState } from "react";

import {
  applyLanguage,
  DICTIONARIES,
  loadLanguage,
  saveLanguage,
  type LanguageId,
} from "./languages";
import { ru, type StringKey } from "./locales/ru";

export type { LanguageId } from "./languages";
export { LANGUAGES } from "./languages";
export type { StringKey } from "./locales/ru";

export type Translate = (
  key: StringKey,
  values?: Readonly<Record<string, string | number>>,
) => string;

/**
 * Fills `{name}` holes. Unknown placeholders are left as written rather than
 * blanked: a visible `{count}` in the UI is a bug report, an empty gap is not.
 */
function format(template: string, values?: Readonly<Record<string, string | number>>): string {
  if (values === undefined) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}

interface I18n {
  readonly language: LanguageId;
  readonly setLanguage: (id: LanguageId) => void;
  readonly t: Translate;
}

/**
 * The default is Russian rather than a throwing stub: a component rendered
 * outside the provider — in a unit test, say — should show words, not crash.
 */
const I18nContext = createContext<I18n>({
  language: "ru",
  setLanguage: () => undefined,
  t: (key, values) => format(ru[key], values),
});

export function I18nProvider({ children }: { readonly children: React.ReactNode }): React.JSX.Element {
  const [language, setLanguageState] = useState<LanguageId>(loadLanguage);

  const setLanguage = useCallback((id: LanguageId): void => {
    setLanguageState(id);
    saveLanguage(id);
    applyLanguage(id);
  }, []);

  const value = useMemo<I18n>(() => {
    const dictionary = DICTIONARIES[language];
    return {
      language,
      setLanguage,
      // A missing key cannot happen — every dictionary is typed against the
      // Russian one — so `t` does no lookup dance beyond the substitution.
      t: (key, values) => format(dictionary[key], values),
    };
  }, [language, setLanguage]);

  return <I18nContext value={value}>{children}</I18nContext>;
}

export function useT(): Translate {
  return useContext(I18nContext).t;
}

export function useLanguage(): readonly [LanguageId, (id: LanguageId) => void] {
  const { language, setLanguage } = useContext(I18nContext);
  return [language, setLanguage];
}
