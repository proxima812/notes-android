import { describe, expect, it } from "vitest";

import { DICTIONARIES, LANGUAGES, LANGUAGE_IDS, isLanguageId, loadLanguage } from "./languages";
import { ru } from "./locales/ru";

const KEYS = Object.keys(ru) as (keyof typeof ru)[];

/** `{name}` holes, which must survive translation or the value goes missing. */
function placeholders(text: string): string[] {
  return [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1] ?? "").sort();
}

describe("dictionaries", () => {
  it("has a picker entry for every language, labelled in itself", () => {
    expect(LANGUAGES.map((language) => language.id)).toEqual([...LANGUAGE_IDS]);
    for (const language of LANGUAGES) {
      expect(language.label.trim()).not.toBe("");
    }
  });

  it("translates every key in every language", () => {
    // The compiler already rejects a missing key; what it cannot see is a key
    // left as an empty string or copied across with the Russian still in it.
    for (const id of LANGUAGE_IDS) {
      const dictionary = DICTIONARIES[id];
      for (const key of KEYS) {
        expect(dictionary[key].trim(), `${id} → ${key}`).not.toBe("");
      }
    }
  });

  it("keeps the placeholders of each phrase", () => {
    for (const id of LANGUAGE_IDS) {
      const dictionary = DICTIONARIES[id];
      for (const key of KEYS) {
        expect(placeholders(dictionary[key]), `${id} → ${key}`).toEqual(placeholders(ru[key]));
      }
    }
  });

  it("leaves nothing in Cyrillic in the Latin-script and Chinese locales", () => {
    // Crimean Tatar is written in Latin here and Chinese in Han: a Cyrillic
    // character in either is a line that was never actually translated.
    for (const id of ["en", "es", "crh", "zh"] as const) {
      for (const key of KEYS) {
        expect(DICTIONARIES[id][key], `${id} → ${key}`).not.toMatch(/[А-Яа-яЁё]/);
      }
    }
  });

  it("recognises only the languages it ships", () => {
    expect(isLanguageId("kk")).toBe(true);
    expect(isLanguageId("de")).toBe(false);
    expect(isLanguageId(null)).toBe(false);
  });

  it("falls back to the default when storage holds an unknown language", () => {
    window.localStorage.setItem("xkeeps.language", "klingon");

    // jsdom reports an English navigator, so an unknown stored value lands on
    // the device language rather than on Russian.
    expect(LANGUAGE_IDS).toContain(loadLanguage());
    window.localStorage.clear();
  });
});
