import { useQuery } from "@tanstack/react-query";
import { AlarmClock, ArrowLeft, Repeat } from "lucide-react";

import { formatDue } from "@/features/reminders/dueLabel";
import {
  listNotesWithReminders,
  type NoteWithReminders,
  type Reminder,
} from "@/features/reminders/api";
import { describeError } from "@/shared/api/errors";
import { useLanguage, useT, type LanguageId, type Translate } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import type { NoteId } from "@/shared/types/ids";

/** When a reminder is due, said the way someone would say it out loud. */
function whenLabel(at: number, language: LanguageId, t: Translate): string {
  const due = formatDue(at, Date.now(), language);
  if (due.day === "today") {
    return t("reminders.today", { time: due.time });
  }
  if (due.day === "tomorrow") {
    return t("reminders.tomorrow", { time: due.time });
  }
  return `${due.date ?? ""}, ${due.time}`;
}

function ReminderLine({
  reminder,
  noteTitle,
}: {
  readonly reminder: Reminder;
  readonly noteTitle: string;
}): React.JSX.Element {
  const t = useT();
  const [language] = useLanguage();

  return (
    <li className="text-content-muted flex items-center gap-2 text-sm">
      <AlarmClock className="text-accent size-4 shrink-0" />
      <span className="shrink-0 tabular-nums">{whenLabel(reminder.scheduledAt, language, t)}</span>
      {/* The reminder's own name only when it is not the note's: the panel
          lets it be edited, and a line repeating the heading above it says
          nothing while taking the width that the time needs. */}
      <span className="min-w-0 flex-1 truncate">
        {reminder.title === noteTitle ? "" : reminder.title}
      </span>
      {reminder.recurrence !== null && (
        <Repeat aria-label={t("reminder.repeat")} className="size-4 shrink-0" />
      )}
    </li>
  );
}

function NoteRow({
  entry,
  onOpen,
}: {
  readonly entry: NoteWithReminders;
  readonly onOpen: (id: NoteId) => void;
}): React.JSX.Element {
  const t = useT();
  const title = entry.note.title === "" ? t("common.untitled") : entry.note.title;

  return (
    <li>
      <button
        type="button"
        aria-label={t("card.open", { title })}
        onClick={() => {
          onOpen(entry.note.id);
        }}
        className="bg-surface-raised border-border-subtle w-full rounded-2xl border p-4 text-left"
      >
        <p className="text-content truncate font-medium">{title}</p>
        <ul className="mt-2 space-y-1">
          {entry.reminders.map((reminder) => (
            <ReminderLine key={reminder.id} reminder={reminder} noteTitle={entry.note.title} />
          ))}
        </ul>
      </button>
    </li>
  );
}

/**
 * Everything the app is going to ask about, on a screen of its own.
 *
 * The library is what someone wrote; this is what is going to happen. They are
 * different questions, and answering the second one by scrolling the first and
 * opening notes to check was the reason this screen exists.
 *
 * A reminder that has already fired is not here: this is a list of what is
 * still ahead, and the note itself is where the past lives.
 */
export function RemindersPage({
  onBack,
  onOpen,
}: {
  readonly onBack: () => void;
  readonly onOpen: (id: NoteId) => void;
}): React.JSX.Element {
  const t = useT();
  const notes = useQuery({
    queryKey: ["notes", "with-reminders"],
    queryFn: () => listNotesWithReminders(),
  });

  useBackGuard(true, onBack);

  return (
    <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-5 p-4">
      <header className="flex items-center gap-1">
        <button
          type="button"
          aria-label={t("common.back")}
          onClick={onBack}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <ArrowLeft className="size-5" />
        </button>
        <h1 className="text-2xl font-semibold tracking-tight">{t("reminders.title")}</h1>
      </header>

      {notes.isPending && <p className="text-content-muted text-sm">{t("common.loading")}</p>}
      {notes.error !== null && (
        <p className="text-danger text-sm">{describeError(notes.error, t)}</p>
      )}
      {notes.data !== undefined && notes.data.items.length === 0 && (
        <p className="text-content-muted text-sm">{t("reminders.empty")}</p>
      )}

      <ul className="flex flex-col gap-2">
        {notes.data?.items.map((entry) => (
          <NoteRow key={entry.note.id} entry={entry} onOpen={onOpen} />
        ))}
      </ul>
    </main>
  );
}
