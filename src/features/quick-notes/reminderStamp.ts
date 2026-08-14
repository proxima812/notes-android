import { FORMATTING_LOCALES } from "@/shared/i18n/formatting";
import type { LanguageId } from "@/shared/i18n";

/**
 * When a freshly dictated reminder will ring, in as few words as possible.
 *
 * The line it goes into is a confirmation the person reads once and then stops
 * looking at, so it says the hour and nothing else while the reminder is today.
 * A date only appears when the answer would otherwise be ambiguous, which is
 * exactly when it earns its space.
 *
 * The date is numeric on purpose. A short month name would be rendered by
 * `Intl` in whichever language the *runtime* falls back to — Bashkir and
 * Crimean Tatar are not locales any WebView carries — and would then sit inside
 * a Bashkir sentence in English. Digits say the same thing in all eight.
 */
export interface ReminderStamp {
  /** `14:30`, always. */
  readonly time: string;
  /** `11.08`, or `null` when the reminder is today. */
  readonly date: string | null;
}

export function formatReminderStamp(
  at: number,
  now: number,
  language: LanguageId,
): ReminderStamp {
  const locale = FORMATTING_LOCALES[language];
  const target = new Date(at);
  const today = new Date(now);

  const time = new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(target);

  if (isSameDay(target, today)) {
    return { time, date: null };
  }

  return {
    time,
    date: new Intl.DateTimeFormat(locale, {
      day: "2-digit",
      month: "2-digit",
    }).format(target),
  };
}

function isSameDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  );
}
