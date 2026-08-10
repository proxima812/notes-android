import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { describeError } from "@/shared/api/errors";
import { useT, type Translate } from "@/shared/i18n";

import {
  loadQuickNoteSettings,
  saveQuickNoteSettings,
  type QuickNoteSettings,
} from "../api";

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
          className="border-border-subtle text-content focus:border-accent min-h-11 rounded-2xl border bg-transparent px-4 text-base outline-none"
        />
      </label>
      <p className="text-content-muted text-xs">{t("quickSettings.fallbackHint")}</p>

      {(settings.error ?? save.error) !== null && (
        <p className="text-danger text-sm">{describeError(settings.error ?? save.error, t)}</p>
      )}
    </section>
  );
}
