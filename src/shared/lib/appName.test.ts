import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_APP_NAME,
  MAX_APP_NAME_LENGTH,
  limitAppName,
  loadAppName,
  loadStoredAppName,
  normaliseAppName,
  saveAppName,
  subscribeAppName,
} from "./appName";

describe("app name", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    window.localStorage.clear();
  });

  it("opens under the shipped name when nothing was ever typed", () => {
    expect(loadStoredAppName()).toBeNull();
    expect(loadAppName()).toBe(DEFAULT_APP_NAME);
    expect([...DEFAULT_APP_NAME]).toHaveLength(MAX_APP_NAME_LENGTH);
  });

  it("round-trips a chosen name", () => {
    saveAppName("Заметки");

    expect(loadAppName()).toBe("Заметки");
  });

  it("cuts a long name to the budget", () => {
    saveAppName("abcdefghijklmnop");

    expect(loadAppName()).toBe("abcdefghij");
  });

  it("counts an emoji as one character rather than two", () => {
    expect(limitAppName("🌙🌙🌙🌙🌙🌙🌙🌙🌙🌙🌙🌙")).toBe("🌙🌙🌙🌙🌙🌙🌙🌙🌙🌙");
  });

  it("keeps the spaces inside a name being typed and drops the ones at its ends", () => {
    expect(limitAppName("my ")).toBe("my ");
    expect(normaliseAppName("  my notes  ")).toBe("my notes");
  });

  it("returns to the default when the field is emptied", () => {
    saveAppName("Дневник");
    saveAppName("   ");

    expect(loadStoredAppName()).toBeNull();
    expect(loadAppName()).toBe(DEFAULT_APP_NAME);
  });

  it("survives storage that throws instead of returning null", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("site data disabled");
    });

    expect(loadAppName()).toBe(DEFAULT_APP_NAME);

    getItem.mockRestore();
  });

  it("tells the header a name changed, until it stops listening", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeAppName(listener);

    saveAppName("Ноты");
    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
    saveAppName("Ноты 2");
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
