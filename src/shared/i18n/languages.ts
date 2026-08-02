import { ba } from "./locales/ba";
import { crh } from "./locales/crh";
import { en } from "./locales/en";
import { es } from "./locales/es";
import { kk } from "./locales/kk";
import { ru, type Dictionary } from "./locales/ru";
import { tt } from "./locales/tt";
import { zh } from "./locales/zh";

/**
 * The languages the app speaks.
 *
 * Each is labelled in itself — someone looking for Bashkir is not helped by the
 * word "Башкирский" in a language they do not read. That is also why the picker
 * shows no flags: a language is not a country, and Tatar, Bashkir and Crimean
 * Tatar have no flag anyone would agree on.
 */

export const LANGUAGE_IDS = ["ru", "en", "es", "kk", "tt", "ba", "crh", "zh"] as const;

export type LanguageId = (typeof LANGUAGE_IDS)[number];

export interface Language {
  readonly id: LanguageId;
  /** Endonym: what speakers call the language. */
  readonly label: string;
}

export const LANGUAGES: readonly Language[] = [
  { id: "ru", label: "Русский" },
  { id: "en", label: "English" },
  { id: "es", label: "Español" },
  { id: "kk", label: "Қазақша" },
  { id: "tt", label: "Татарча" },
  { id: "ba", label: "Башҡортса" },
  { id: "crh", label: "Qırımtatarca" },
  { id: "zh", label: "中文" },
];

export const DICTIONARIES: Readonly<Record<LanguageId, Dictionary>> = {
  ru,
  en,
  es,
  kk,
  tt,
  ba,
  crh,
  zh,
};

export const DEFAULT_LANGUAGE_ID: LanguageId = "ru";

const STORAGE_KEY = "xkeeps.language";

export function isLanguageId(value: string | null): value is LanguageId {
  return value !== null && (LANGUAGE_IDS as readonly string[]).includes(value);
}

/**
 * The language the phone is set to, if the app speaks it.
 *
 * Matched on the primary subtag, so `es-MX` and `zh-Hans-CN` land on Spanish and
 * Chinese instead of falling through to Russian.
 */
function fromDevice(): LanguageId | null {
  const tags = typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]);
  for (const tag of tags) {
    const primary = tag.toLowerCase().split("-")[0] ?? "";
    if (isLanguageId(primary)) {
      return primary;
    }
  }
  return null;
}

/**
 * Reading storage is wrapped because a WebView with site data disabled throws
 * here rather than returning null, and losing the saved language must never take
 * the whole app down with it.
 */
export function loadLanguage(): LanguageId {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (isLanguageId(stored)) {
      return stored;
    }
  } catch {
    // Fall through to the device's own choice.
  }
  return fromDevice() ?? DEFAULT_LANGUAGE_ID;
}

export function saveLanguage(id: LanguageId): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // A language that survives only this session still beats a crash.
  }
}

/** Tells the WebView which language it is rendering: hyphenation, fonts, voice. */
export function applyLanguage(id: LanguageId): void {
  document.documentElement.lang = id;
}
