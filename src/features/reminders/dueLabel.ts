import { FORMATTING_LOCALES } from "@/shared/i18n/formatting";
import type { LanguageId } from "@/shared/i18n";

/**
 * When a reminder is due, split so the screen can put it into a sentence.
 *
 * `day` is `"today"` or `"tomorrow"` when the reminder falls on one of them,
 * and those two are named rather than dated: a list of what is about to happen
 * is read at a glance, and "11.08" for tonight is a date to decode instead of
 * an answer. Anything further off carries a date instead.
 *
 * The date is numeric for the same reason the dictation stamp's is: `Intl` has
 * no Bashkir or Crimean Tatar, so a short month name would arrive in whatever
 * language the WebView fell back to and sit inside a Bashkir line in English.
 */
export interface DueLabel {
  /** `19:00`, always. */
  readonly time: string;
  readonly day: "today" | "tomorrow" | null;
  /** `13.08`, or `null` when `day` says it. */
  readonly date: string | null;
}

export function formatDue(at: number, now: number, language: LanguageId): DueLabel {
  const locale = FORMATTING_LOCALES[language];
  const target = new Date(at);

  const time = new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(target);

  // Counted in whole days between midnights rather than in hours, so 23:00 and
  // 01:00 are a day apart the way a person means it.
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const days = Math.floor((startOfDay(target) - midnight.getTime()) / 86_400_000);

  if (days === 0) {
    return { time, day: "today", date: null };
  }
  if (days === 1) {
    return { time, day: "tomorrow", date: null };
  }

  return {
    time,
    day: null,
    date: new Intl.DateTimeFormat(locale, {
      day: "2-digit",
      month: "2-digit",
    }).format(target),
  };
}

function startOfDay(value: Date): number {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
}
