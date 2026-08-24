import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { describeError } from "@/shared/api/errors";
import { useT, type Translate } from "@/shared/i18n";

import { loadSnoozeSetting, saveSnoozeSetting, type SnoozeSetting } from "../api";

function amountLabel(minutes: number, t: Translate): string {
  if (minutes % 60 === 0) {
    return t("snooze.hours", { count: minutes / 60 });
  }
  return t("snooze.minutes", { count: minutes });
}

/**
 * How long the notification's "later" button moves a reminder by.
 *
 * One setting for the whole app rather than one per reminder: "later" is how
 * long this person needs before being asked again, and asking it once per
 * reminder would be a question nobody wants to answer twice.
 *
 * Saving re-arms every alarm already waiting, because the amount travels inside
 * the alarm the OS holds — the notification is posted by a receiver that may
 * run with the app dead and cannot look anything up.
 */
export function SnoozeSettingSection(): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();

  const setting = useQuery({ queryKey: ["snooze-setting"], queryFn: loadSnoozeSetting });

  const save = useMutation({
    mutationFn: (minutes: number) => saveSnoozeSetting(minutes),
    onSuccess: (saved: SnoozeSetting) => {
      client.setQueryData(["snooze-setting"], saved);
    },
  });

  // While the re-arming is in flight the row shows what was asked for rather
  // than what is stored: a chip that snaps back for a moment reads as refusal.
  const current = save.isPending
    ? { ...setting.data, minutes: save.variables }
    : setting.data;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-content-muted text-sm font-medium">{t("snooze.title")}</h2>

      <div
        role="radiogroup"
        aria-label={t("snooze.title")}
        className="-mx-4 flex gap-2 overflow-x-auto px-4 pb-1"
      >
        {(setting.data?.offered ?? []).map((minutes) => {
          const chosen = current?.minutes === minutes;
          return (
            <button
              key={minutes}
              type="button"
              role="radio"
              aria-checked={chosen}
              disabled={setting.data === undefined || save.isPending}
              onClick={() => {
                save.mutate(minutes);
              }}
              className={`min-h-11 shrink-0 rounded-xl px-3 text-sm disabled:opacity-40 ${
                chosen ? "bg-accent text-accent-content" : "bg-surface-raised text-content-muted"
              }`}
            >
              {amountLabel(minutes, t)}
            </button>
          );
        })}
      </div>

      <p className="text-content-muted text-xs">{t("snooze.hint")}</p>

      {(setting.error ?? save.error) !== null && (
        <p className="text-danger text-sm">{describeError(setting.error ?? save.error, t)}</p>
      )}
    </section>
  );
}
