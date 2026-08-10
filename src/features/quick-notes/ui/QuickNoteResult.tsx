import { useMutation } from "@tanstack/react-query";
import { Settings, X } from "lucide-react";
import { useState } from "react";

import { trashNote } from "@/features/notes/api";
import { upsertReminderForNote, type Reminder } from "@/features/reminders/api";
import { openAppSettings } from "@/features/quick-notes/speech";
import { AppError, describeError } from "@/shared/api/errors";
import { useLanguage, useT, type Translate } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import type { QuickNote } from "../api";
import { formatReminderStamp } from "../reminderStamp";

/**
 * The shifts offered when a dictated time came out wrong, in minutes.
 *
 * A mishearing is usually a mishearing of the hour, and everything else is
 * closer than it looks: the person is standing there with the phone in one hand
 * and wants the alarm nudged, not a date picker.
 */
const SHIFTS = [-60, -15, 15, 60] as const;

interface QuickNoteResultProps {
  readonly dictated: QuickNote;
  readonly onOpen: (id: NoteId) => void;
  readonly onDismiss: () => void;
  /** Re-read the library: the note was removed or its reminder moved. */
  readonly onChanged: () => void;
}

/**
 * What the dictation produced, and the two ways it can be wrong.
 *
 * It is a line rather than a screen because the common case is that everything
 * is right and the person is already walking away. But the two failures speech
 * actually has — the wrong words and the wrong hour — both cost one tap here
 * instead of a trip through the editor: «Отменить» takes the note back, and the
 * shifts move the alarm without opening anything.
 *
 * The line is written in the speaker's frame. «Встреча — сказали 15:00,
 * напомню в 14:30» reads as a correct parse; the alarm time alone reads as a
 * mishearing of the time that was just said out loud.
 */
export function QuickNoteResult({
  dictated,
  onOpen,
  onDismiss,
  onChanged,
}: QuickNoteResultProps): React.JSX.Element {
  const t = useT();
  const [language] = useLanguage();
  const [reminder, setReminder] = useState<Reminder | null>(dictated.reminder);
  const [error, setError] = useState<string | null>(null);

  const undo = useMutation({
    mutationFn: () => trashNote(dictated.note.id),
    onSuccess: () => {
      // The note goes to the bin rather than being purged: undoing a dictation
      // is not the same as being sure it was rubbish.
      onChanged();
      onDismiss();
    },
    onError: (cause: unknown) => {
      setError(describeError(cause, t));
    },
  });

  const shift = useMutation({
    mutationFn: async (minutes: number): Promise<Reminder> => {
      if (reminder === null) {
        throw new Error("no reminder to move");
      }
      return upsertReminderForNote({
        reminderId: reminder.id,
        noteId: reminder.noteId,
        title: reminder.title,
        body: reminder.body,
        scheduledAt: reminder.scheduledAt + minutes * 60_000,
        timezone: reminder.timezone,
        sound: reminder.sound,
        recurrence: reminder.recurrence,
      });
    },
    onSuccess: (moved) => {
      setReminder(moved);
      setError(null);
      onChanged();
    },
    onError: (cause: unknown) => {
      setError(describeError(cause, t));
    },
  });

  const failure =
    dictated.reminderError === null ? null : new AppError(dictated.reminderError);

  return (
    <div className="border-border-subtle flex flex-col gap-2 rounded-2xl border p-3">
      <div className="flex items-start gap-2">
        <p className="text-content min-w-0 flex-1 text-sm">
          {summary(dictated, reminder, language, t)}
        </p>
        <button
          type="button"
          aria-label={t("quick.dismiss")}
          onClick={onDismiss}
          className="text-content-muted -m-1 shrink-0 p-1"
        >
          <X className="size-4" />
        </button>
      </div>

      {/* The core knows why no alarm was armed. Saying «без напоминания» and
          dropping the reason would leave the one fact that can fix it in a log
          nobody reads. */}
      {failure !== null && <p className="text-danger text-sm">{failure.message}</p>}
      {error !== null && <p className="text-danger text-sm">{error}</p>}

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => {
            onOpen(dictated.note.id);
          }}
          className="text-accent min-h-9 text-sm font-medium"
        >
          {t("quick.open")}
        </button>

        <button
          type="button"
          disabled={undo.isPending}
          onClick={() => {
            undo.mutate();
          }}
          className="text-content-muted min-h-9 text-sm disabled:opacity-40"
        >
          {t("quick.undo")}
        </button>

        {reminder !== null &&
          SHIFTS.map((minutes) => (
            <button
              key={minutes}
              type="button"
              disabled={shift.isPending}
              onClick={() => {
                shift.mutate(minutes);
              }}
              className="border-border-subtle text-content-muted min-h-9 rounded-xl border px-2 text-xs tabular-nums disabled:opacity-40"
            >
              {minutes > 0 ? `+${shiftLabel(minutes, t)}` : `−${shiftLabel(-minutes, t)}`}
            </button>
          ))}

        {failure?.isActionable === true && (
          <button
            type="button"
            onClick={() => {
              openAppSettings().catch(() => undefined);
            }}
            className="text-accent flex min-h-9 items-center gap-1 text-sm font-medium"
          >
            <Settings className="size-4" />
            {t("quick.openSettings")}
          </button>
        )}
      </div>
    </div>
  );
}

function shiftLabel(minutes: number, t: Translate): string {
  return minutes % 60 === 0
    ? t("quickSettings.hours", { count: minutes / 60 })
    : t("quickSettings.minutes", { count: minutes });
}

/**
 * One sentence covering the three outcomes: an alarm before a time that was
 * said, an alarm at a time nobody said, and no alarm at all.
 */
function summary(
  dictated: QuickNote,
  reminder: Reminder | null,
  language: ReturnType<typeof useLanguage>[0],
  t: Translate,
): string {
  const title = dictated.note.title;
  if (reminder === null) {
    return t("quick.createdWithoutReminder", { title });
  }

  const now = Date.now();
  const alarm = formatReminderStamp(reminder.scheduledAt, now, language);
  const when = alarm.date === null ? alarm.time : `${alarm.date}, ${alarm.time}`;

  // The lead only earns a mention when there is a spoken time for it to be
  // early against — otherwise «напомню в 19:00» is the whole truth.
  if (dictated.leadMinutes > 0 && dictated.spokenAt !== null) {
    const spoken = formatReminderStamp(dictated.spokenAt, now, language);
    return t("quick.createdWithLead", { title, spoken: spoken.time, when });
  }

  return alarm.date === null
    ? t("quick.created", { title, when })
    : t("quick.createdLater", { title, when });
}
