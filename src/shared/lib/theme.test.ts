import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  APP_THEMES,
  APP_THEME_IDS,
  DEFAULT_THEME_ID,
  applyTheme,
  isThemeId,
  loadStoredTheme,
  loadTheme,
  saveTheme,
} from "./theme";

describe("app themes", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    window.localStorage.clear();
    delete document.documentElement.dataset["theme"];
  });

  it("offers a preview for every theme the CSS defines", () => {
    expect(APP_THEMES.map((theme) => theme.id)).toEqual([...APP_THEME_IDS]);
    for (const theme of APP_THEMES) {
      expect(theme.swatch).toContain("linear-gradient");
      expect(theme.labelKey).not.toBe("");
    }
  });

  it("falls back to the default when storage holds an unknown id", () => {
    window.localStorage.setItem("xkeeps.theme", "chartreuse");

    expect(loadStoredTheme()).toBeNull();
    expect(loadTheme()).toBe(DEFAULT_THEME_ID);
  });

  it("opens in the default when nothing was ever picked", () => {
    expect(loadTheme()).toBe(DEFAULT_THEME_ID);
    expect(DEFAULT_THEME_ID).toBe("obsidian");
  });

  it("has no light theme left to fall back to", () => {
    expect(APP_THEME_IDS).not.toContain("porcelain");
  });

  it("round-trips a saved theme", () => {
    saveTheme("amethyst");

    expect(loadTheme()).toBe("amethyst");
  });

  it("survives storage that throws instead of returning null", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("site data disabled");
    });

    expect(loadTheme()).toBe(DEFAULT_THEME_ID);

    getItem.mockRestore();
  });

  it("drives the attribute the stylesheet selects on", () => {
    applyTheme("amethyst");

    expect(document.documentElement.dataset["theme"]).toBe("amethyst");
  });

  it("rejects a non-theme string", () => {
    expect(isThemeId("mint")).toBe(true);
    expect(isThemeId("blue")).toBe(false);
    expect(isThemeId(null)).toBe(false);
  });
});
