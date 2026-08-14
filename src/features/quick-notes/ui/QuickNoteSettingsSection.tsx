import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { describeError } from "@/shared/api/errors";
import { useT, type StringKey, type Translate } from "@/shared/i18n";
import { PickerField, PICKER_CLASSES } from "@/shared/ui/PickerField";

import {
  loadQuickNoteSettings,
  saveQuickNoteSettings,
  type QuickNoteSettings,
} from "../api";

/** The first day that is neither today nor tomorrow, so needs saying in full. */
const CUSTOM_FROM = 2;

/**
 * The three answers to "which day".
 *
 * Today and tomorrow are single numbers; "own" is every number from the day
 * after tomorrow onwards, which is why it is a range rather than a value and
 * keeps whatever the user last typed when it is chosen again.
 */
const DAY_CHOICES = [
  {
    id: "today",
    labelKey: "quickSettings.dayToday",
    matches: (offset: number) => offset === 0,
    offsetFrom: () => 0,
  },
  {
    id: "tomorrow",
    labelKey: "quickSettings.dayTomorrow",
    matches: (offset: number) => offset === 1,
    offsetFrom: () => 1,
  },
  {
    id: "own",
    labelKey: "quickSettings.dayOwn",
    matches: (offset: number) => offset >= CUSTOM_FROM,
    offsetFrom: (offset: number) => (offset >= CUSTOM_FROM ? offset : CUSTOM_FROM),
  },
] as const satisfies readonly {
  id: string;
  labelKey: StringKey;
  matches: (offset: number) => boolean;
  offsetFrom: (offset: number) => number;
}[];

function leadLabel(minutes: number, t: Translate): string {
  if (minutes === 0) {
    return t("quickSettings.leadNone");
  }
  if (minutes % 60 === 0) {
    return t("quickSettings.hours", { count: minutes / 60 });
  }
  return t("quickSettings.minutes", { count: minutes });
}

/**
 * Everything a dictated note needs that the sentence cannot say.
 *
 * Both settings are written the moment they are touched, like the rest of this
 * screen: there is nothing to confirm, and the example under each one already
 * shows what the choice does.
 */
export function QuickNoteSettingsSection(): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();

  const settings = useQuery({
    queryKey: ["quick-note-settings"],
    queryFn: loadQuickNoteSettings,
  });

  const save = useMutation({
    mutationFn: (next: QuickNoteSettings) => saveQuickNoteSettings(next),
    onSuccess: (saved) => {
      client.setQueryData(["quick-note-settings"], saved);
    },
  });

  // While a change is in flight the chips show what was asked for rather than
  // what is stored: a row that snaps back for one frame reads as a rejection.
  const current = save.isPending ? save.variables : settings.data;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-content-muted text-sm font-medium">{t("quickSettings.title")}</h2>

      {/* Which amounts are worth offering is a product decision, so the list
          comes from the core the way the reminder time presets do — including
          a stored value that is not among them, which would otherwise leave
          every chip unselected. */}
      <p className="text-content text-sm">{t("quickSettings.lead")}</p>
      <div
        role="radiogroup"
        aria-label={t("quickSettings.lead")}
        className="-mx-4 flex gap-2 overflow-x-auto px-4 pb-1"
      >
        {(current?.offeredLeads ?? []).map((minutes) => {
          const chosen = current?.leadMinutes === minutes;
          return (
            <button
              key={minutes}
              type="button"
              role="radio"
              aria-checked={chosen}
              disabled={current === undefined}
              onClick={() => {
                if (current !== undefined) {
                  save.mutate({ ...current, leadMinutes: minutes });
                }
              }}
              className={`min-h-11 shrink-0 rounded-xl border px-3 text-sm ${
                chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
              }`}
            >
              {leadLabel(minutes, t)}
            </button>
          );
        })}
      </div>
      <p className="text-content-muted text-xs">{t("quickSettings.leadHint")}</p>

      <label className="flex flex-col gap-2">
        <span className="text-content text-sm">{t("quickSettings.fallback")}</span>
        <PickerField>
          <input
            type="time"
            value={current?.fallbackTime ?? ""}
            disabled={current === undefined}
            onChange={(event) => {
              // An emptied time input reports `""`, and the core has no way to
              // store "no hour" — the field keeps the last real value until
              // another one is picked.
              if (current !== undefined && event.target.value !== "") {
                save.mutate({ ...current, fallbackTime: event.target.value });
              }
            }}
            className={`${PICKER_CLASSES} disabled:opacity-40`}
          />
        </PickerField>
      </label>
      <p className="text-content-muted text-xs">{t("quickSettings.fallbackHint")}</p>

      {/* The day that hour lands on. Today and tomorrow are the two answers
          almost everyone has; the third is for someone whose errands are
          always a few days out, and it is a number rather than a date because
          a date would go stale the day after it was picked. */}
      <p className="text-content text-sm">{t("quickSettings.day")}</p>
      <div role="radiogroup" aria-label={t("quickSettings.day")} className="flex gap-2">
        {DAY_CHOICES.map((choice) => {
          const chosen = choice.matches(current?.fallbackDayOffset ?? 0);
          return (
            <button
              key={choice.id}
              type="button"
              role="radio"
              aria-checked={chosen}
              disabled={current === undefined}
              onClick={() => {
                if (current !== undefined) {
                  save.mutate({
                    ...current,
                    // Choosing "own" has to land on a day, and the first one
                    // the other two chips do not already cover is the day
                    // after tomorrow. Coming back to it keeps what was typed.
                    fallbackDayOffset: choice.offsetFrom(current.fallbackDayOffset),
                  });
                }
              }}
              className={`min-h-11 flex-1 rounded-xl border px-3 text-sm ${
                chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
              }`}
            >
              {t(choice.labelKey)}
            </button>
          );
        })}
      </div>

      {current !== undefined && current.fallbackDayOffset >= CUSTOM_FROM && (
        <label className="flex items-center gap-3">
          <span className="text-content-muted flex-1 text-sm">{t("quickSettings.dayCustom")}</span>
          <input
            type="number"
            inputMode="numeric"
            min={CUSTOM_FROM}
            max={current.maxFallbackDayOffset}
            value={current.fallbackDayOffset}
            onChange={(event) => {
              const days = Number.parseInt(event.target.value, 10);
              // An emptied field reports `""`; the number keeps its last real
              // value until another one is typed, the way the hour does.
              if (Number.isInteger(days) && days >= CUSTOM_FROM && days <= current.maxFallbackDayOffset) {
                save.mutate({ ...current, fallbackDayOffset: days });
              }
            }}
            className="border-border-subtle text-content focus:border-accent min-h-11 w-20 rounded-xl border bg-transparent px-3 text-base tabular-nums outline-none"
          />
        </label>
      )}
      <p className="text-content-muted text-xs">{t("quickSettings.dayHint")}</p>

      {(settings.error ?? save.error) !== null && (
        <p className="text-danger text-sm">{describeError(settings.error ?? save.error, t)}</p>
      )}
    </section>
  );
}
