import { describe, expect, it } from "vitest";

import { formatDue } from "./dueLabel";

/** 13 August 2026, 21:00 local. */
const NOW = new Date(2026, 7, 13, 21, 0).getTime();

function at(day: number, hour: number, minute = 0): number {
  return new Date(2026, 7, day, hour, minute).getTime();
}

describe("formatDue", () => {
  it("names today rather than dating it", () => {
    expect(formatDue(at(13, 22, 30), NOW, "ru")).toEqual({
      time: "22:30",
      day: "today",
      date: null,
    });
  });

  it("counts days between midnights, not hours", () => {
    // Two hours ahead, but past midnight: a person says "tomorrow" here, and
    // an hours-based answer would say "today".
    expect(formatDue(at(14, 1), NOW, "ru").day).toBe("tomorrow");
  });

  it("dates anything further off", () => {
    const due = formatDue(at(20, 9), NOW, "ru");
    expect(due.day).toBeNull();
    expect(due.date).toBe("20.08");
    expect(due.time).toBe("09:00");
  });

  it("keeps a twenty-four-hour clock in a locale that would otherwise use am", () => {
    // The rest of the app shows 24-hour times, and a single row reading
    // "9:00 pm" among them would look like a different kind of value.
    expect(formatDue(at(13, 21, 5), NOW, "en").time).toBe("21:05");
  });

  it("formats dates for languages Intl does not ship, without falling back to English", () => {
    // Bashkir and Crimean Tatar are not locales any WebView carries, so the
    // date has to be digits — a month name would arrive in English.
    for (const language of ["ba", "crh", "tt"] as const) {
      expect(formatDue(at(20, 9), NOW, language).date).toMatch(/^\d{2}[.\/]\d{2}$/);
    }
  });
});
