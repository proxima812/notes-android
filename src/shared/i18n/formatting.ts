import type { LanguageId } from "./languages";

/**
 * The locale each interface language formats numbers and dates with.
 *
 * Kept separate from `LanguageId` because that is the app's own list, and three
 * of its entries are languages no `Intl` implementation ships. Those borrow the
 * conventions of the language their speakers read dates in, which is a claim
 * about calendars rather than about people.
 */
export const FORMATTING_LOCALES: Readonly<Record<LanguageId, string>> = {
  ru: "ru-RU",
  en: "en-GB",
  es: "es-ES",
  kk: "kk-KZ",
  tt: "ru-RU",
  ba: "ru-RU",
  crh: "uk-UA",
  zh: "zh-CN",
};
