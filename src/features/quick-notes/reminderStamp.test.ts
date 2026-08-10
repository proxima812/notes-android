import { describe, expect, it } from "vitest";

import { formatReminderStamp } from "./reminderStamp";
import { chooseRecognitionTag, recognitionTag, speechErrorCode } from "./speech";

/** 10 August 2026, 12:00 local. */
const NOON = new Date(2026, 7, 10, 12, 0).getTime();

describe("formatReminderStamp", () => {
  it("says only the hour while the reminder is today", () => {
    const at = new Date(2026, 7, 10, 14, 30).getTime();
    expect(formatReminderStamp(at, NOON, "ru")).toEqual({ time: "14:30", date: null });
  });

  it("adds a numeric date once the reminder is another day", () => {
    const at = new Date(2026, 7, 11, 9, 5).getTime();
    const stamp = formatReminderStamp(at, NOON, "ru");
    expect(stamp.time).toBe("09:05");
    expect(stamp.date).toMatch(/11/);
  });

  it("never spells a month, so no sentence ends up half in another language", () => {
    // Bashkir is not a locale any WebView carries. A short month name would be
    // rendered in whatever the runtime falls back to and then sit inside a
    // Bashkir sentence.
    const at = new Date(2026, 7, 11, 9, 5).getTime();
    for (const language of ["ba", "tt", "crh", "en", "zh"] as const) {
      const stamp = formatReminderStamp(at, NOON, language);
      expect(stamp.date, language).toMatch(/^[\d./\-\s]+$/u);
    }
  });

  it("keeps a 24-hour clock in a locale that would otherwise use am and pm", () => {
    const at = new Date(2026, 7, 10, 21, 0).getTime();
    expect(formatReminderStamp(at, NOON, "en").time).toBe("21:00");
  });

  it("does not mistake the same hour a year later for today", () => {
    const at = new Date(2027, 7, 10, 12, 0).getTime();
    expect(formatReminderStamp(at, NOON, "ru").date).not.toBeNull();
  });
});

describe("chooseRecognitionTag", () => {
  const unknown = { known: false, installed: [], supported: [] };

  it("keeps the interface language when the device cannot be asked", () => {
    // Before Android 13 there is no way to know. Guessing on no information
    // would break the majority case to spare the minority one.
    expect(chooseRecognitionTag("ru", ["ru-RU"], unknown)).toEqual({
      tag: "ru-RU",
      reason: "unknown",
    });
  });

  it("uses the interface language when the device has that model", () => {
    expect(
      chooseRecognitionTag("es", ["en-GB"], {
        known: true,
        installed: ["es-ES", "en-GB"],
        supported: [],
      }),
    ).toEqual({ tag: "es-ES", reason: "ui" });
  });

  it("falls back to the phone's own language when the interface one cannot be heard", () => {
    // Reading the app in Tatar and dictating in Russian is the real case: no
    // recogniser anywhere ships Tatar.
    expect(
      chooseRecognitionTag("tt", ["ru-RU", "en-US"], {
        known: true,
        installed: ["ru-RU", "en-US"],
        supported: [],
      }),
    ).toEqual({ tag: "ru-RU", reason: "device" });
  });

  it("falls back to Russian when neither the interface nor the phone can be heard", () => {
    expect(
      chooseRecognitionTag("ba", ["ba-RU"], {
        known: true,
        installed: ["ru-RU", "de-DE"],
        supported: [],
      }),
    ).toEqual({ tag: "ru-RU", reason: "fallback" });
  });

  it("matches on the language even when the regions differ", () => {
    expect(
      chooseRecognitionTag("en", [], {
        known: true,
        installed: ["en-GB"],
        supported: [],
      }),
    ).toEqual({ tag: "en-GB", reason: "ui" });
  });

  it("prefers a model already on the device over one that would be downloaded", () => {
    expect(
      chooseRecognitionTag("es", [], {
        known: true,
        installed: ["ru-RU"],
        supported: ["es-ES"],
      }),
    ).toEqual({ tag: "ru-RU", reason: "fallback" });
  });

  it("takes whatever the device can do rather than a tag it has said no to", () => {
    expect(
      chooseRecognitionTag("zh", [], {
        known: true,
        installed: ["de-DE"],
        supported: [],
      }),
    ).toEqual({ tag: "de-DE", reason: "fallback" });
  });
});

describe("recognitionTag", () => {
  it("names a region, because a bare language tag recognises badly", () => {
    expect(recognitionTag("ru")).toBe("ru-RU");
    expect(recognitionTag("zh")).toBe("zh-CN");
  });
});

describe("speechErrorCode", () => {
  it("passes through the codes this build has a sentence for", () => {
    expect(speechErrorCode("no_speech")).toBe("no_speech");
    expect(speechErrorCode("language")).toBe("language");
  });

  it("folds anything else into one code rather than showing it raw", () => {
    expect(speechErrorCode("ERROR_17")).toBe("unknown");
    expect(speechErrorCode("")).toBe("unknown");
  });
});
