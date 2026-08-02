import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlarmClock, ArrowLeft, Palette } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { getNote, updateNote, type UpdateNoteRequest } from "@/features/notes/api";
import { RichTextEditor, type EditorSnapshot } from "@/features/notes/editor/RichTextEditor";
import { GradientPicker } from "@/features/notes/ui/GradientPicker";
import {
  deleteReminder,
  listRemindersForNote,
  listReminderSounds,
  listReminderTimePresets,
  saveReminderTimePresets,
  upsertReminderForNote,
} from "@/features/reminders/api";
import { ReminderPanel } from "@/features/reminders/ui/ReminderPanel";
import type { Reminder } from "@/features/reminders/api";
import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import { findGradient } from "@/shared/lib/gradients";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import { deviceTimeZone, type NoteId } from "@/shared/types/ids";

/** Idle time before an edit is persisted. Long enough to coalesce typing. */
const AUTOSAVE_DELAY_MS = 600;

interface NoteEditorPageProps {
  readonly id: NoteId;
  readonly onBack: () => void;
}

export function NoteEditorPage({ id, onBack }: NoteEditorPageProps): React.JSX.Element {
  const client = useQueryClient();
  const t = useT();
  const note = useQuery({ queryKey: ["note", id], queryFn: () => getNote(id) });

  const save = useMutation({
    mutationFn: (patch: UpdateNoteRequest) => updateNote(id, patch),
  });

  // The pending patch lives in a ref so that typing does not re-render the
  // screen on every keystroke; only the timer reads it.
  const pending = useRef<UpdateNoteRequest>({});
  const timer = useRef<number | null>(null);
  const { mutate } = save;

  const flush = useCallback((): void => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    if (Object.keys(pending.current).length === 0) {
      return;
    }
    const patch = pending.current;
    pending.current = {};
    mutate(patch);
  }, [mutate]);

  const schedule = useCallback(
    (patch: UpdateNoteRequest): void => {
      pending.current = { ...pending.current, ...patch };
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
      timer.current = window.setTimeout(flush, AUTOSAVE_DELAY_MS);
    },
    [flush],
  );

  // Leaving the screen must not lose the last few hundred milliseconds of
  // typing, so the unmount flushes whatever the timer has not written yet.
  useEffect(() => flush, [flush]);

  const leave = (): void => {
    flush();
    void client.invalidateQueries({ queryKey: ["notes"] });
    void client.invalidateQueries({ queryKey: ["app-info"] });
    onBack();
  };

  // The back gesture is how most people leave a screen on Android, so it has to
  // run the same exit as the arrow — flush included — not close the app.
  useBackGuard(true, leave);

  if (note.isPending) {
    return <p className="text-content-muted p-4 text-sm">{t("common.loading")}</p>;
  }
  if (note.error !== null) {
    return <p className="text-danger p-4 text-sm">{describeError(note.error, t)}</p>;
  }
  if (note.data === null) {
    return <p className="text-content-muted p-4 text-sm">{t("editor.notFound")}</p>;
  }

  return <Loaded note={note.data} onLeave={leave} onPatch={schedule} saving={save.isPending} />;
}

interface LoadedProps {
  readonly note: NonNullable<Awaited<ReturnType<typeof getNote>>>;
  readonly onLeave: () => void;
  readonly onPatch: (patch: UpdateNoteRequest) => void;
  readonly saving: boolean;
}

/**
 * The editor proper, mounted only once the note exists.
 *
 * Splitting it out is what lets the title and body start from server data
 * without the inputs being re-seeded on every refetch.
 */
function Loaded({ note, onLeave, onPatch, saving }: LoadedProps): React.JSX.Element {
  const client = useQueryClient();
  const t = useT();
  const [title, setTitle] = useState(note.title);
  const [currentContentText, setCurrentContentText] = useState(note.contentText);
  const [color, setColor] = useState(note.color);
  const [showColors, setShowColors] = useState(false);
  const [showReminder, setShowReminder] = useState(false);

  const reminders = useQuery({
    queryKey: ["reminders", note.id],
    queryFn: () => listRemindersForNote(note.id),
  });
  /** The reminder being edited, or `null` while adding a new one. */
  const [editing, setEditing] = useState<Reminder | null>(null);
  const sounds = useQuery({
    queryKey: ["reminder-sounds"],
    queryFn: listReminderSounds,
  });
  const timePresets = useQuery({
    queryKey: ["reminder-time-presets"],
    queryFn: listReminderTimePresets,
  });
  const saveTimePresets = useMutation({
    mutationFn: saveReminderTimePresets,
    // The core is what sorts and deduplicates, so the answer it gives is the
    // list to show — writing the request into the cache would briefly display
    // an order the user is about to see change.
    onSuccess: (next) => {
      client.setQueryData(["reminder-time-presets"], next);
    },
  });
  const saveReminder = useMutation({
    mutationFn: upsertReminderForNote,
    onSuccess: async () => {
      setEditing(null);
      setShowReminder(false);
      await client.invalidateQueries({ queryKey: ["reminders", note.id] });
    },
  });
  const removeReminder = useMutation({
    mutationFn: deleteReminder,
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ["reminders", note.id] });
    },
  });

  const gradient = findGradient(color);

  // Registered after the screen's own guard, so back closes an open panel first
  // and only then leaves the note.
  useBackGuard(showReminder, () => {
    setShowReminder(false);
  });
  useBackGuard(showColors, () => {
    setShowColors(false);
  });

  const onBody = useCallback(
    (snapshot: EditorSnapshot): void => {
      setCurrentContentText(snapshot.contentText);
      onPatch({ contentText: snapshot.contentText, contentJson: snapshot.contentJson });
    },
    [onPatch],
  );

  return (
    <div
      // A coloured note paints the whole editor; the ink and the link colour
      // come with `note-surface`, per theme.
      data-note={gradient === null ? undefined : gradient.id}
      className={`flex flex-1 flex-col ${gradient === null ? "" : "note-surface"}`}
    >
      <header className="flex items-center gap-1 p-2">
        <button
          type="button"
          aria-label={t("common.back")}
          onClick={onLeave}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <ArrowLeft className="size-5" />
        </button>

        <span className="text-content-muted flex-1 text-center text-xs">
          {saving ? t("editor.saving") : t("editor.saved")}
        </span>

        <button
          type="button"
          aria-label={t("reminder.title")}
          aria-pressed={showReminder}
          onClick={() => {
            setShowReminder((open) => !open);
          }}
          className={`flex size-11 shrink-0 items-center justify-center rounded-full ${
            reminders.data === undefined || reminders.data.length === 0
              ? "text-content"
              : "text-accent"
          }`}
        >
          <AlarmClock className="size-5" />
        </button>

        <button
          type="button"
          aria-label={t("editor.color")}
          aria-pressed={showColors}
          onClick={() => {
            setShowColors((open) => !open);
          }}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <Palette className="size-5" />
        </button>
      </header>

      {showReminder && (
        <div className="px-4 pb-2">
          {reminders.isPending || sounds.isPending || timePresets.isPending ? (
            <p className="text-content-muted py-3 text-sm">{t("reminder.loading")}</p>
          ) : reminders.error !== null || sounds.error !== null || timePresets.error !== null ? (
            <p className="text-danger py-3 text-sm">
              {describeError(reminders.error ?? sounds.error ?? timePresets.error, t)}
            </p>
          ) : reminders.data !== undefined &&
            sounds.data !== undefined &&
            timePresets.data !== undefined ? (
            <ReminderPanel
              initial={editing}
              existing={reminders.data}
              onEdit={setEditing}
              onRemove={(id) => {
                removeReminder.mutate(id);
              }}
              sounds={sounds.data}
              noteTitle={title}
              presets={timePresets.data}
              onSavePresets={(next) => {
                saveTimePresets.mutate(next);
              }}
              busy={saveReminder.isPending || removeReminder.isPending}
              error={
                saveReminder.error !== null
                  ? describeError(saveReminder.error, t)
                  : removeReminder.error !== null
                    ? describeError(removeReminder.error, t)
                    : null
              }
              onSave={(value) => {
                saveReminder.mutate({
                  reminderId: editing?.id ?? null,
                  noteId: note.id,
                  title: value.title,
                  body: currentContentText,
                  scheduledAt: value.scheduledAt,
                  timezone: deviceTimeZone(),
                  sound: value.sound,
                  recurrence: value.recurrence,
                });
              }}
              onClose={() => {
                setEditing(null);
                setShowReminder(false);
              }}
            />
          ) : null}
        </div>
      )}

      {showColors && (
        <div className="px-4 pb-2">
          <GradientPicker
            value={color}
            onChange={(next) => {
              setColor(next);
              onPatch({ color: next });
            }}
          />
        </div>
      )}

      <div className="flex flex-1 flex-col gap-2 px-4 pb-2">
        <input
          type="text"
          value={title}
          onChange={(event) => {
            setTitle(event.target.value);
            onPatch({ title: event.target.value });
          }}
          placeholder={t("editor.titlePlaceholder")}
          aria-label={t("editor.titlePlaceholder")}
          className="min-h-12 w-full bg-transparent text-2xl font-semibold tracking-tight outline-none"
        />

        <RichTextEditor
          initialJson={note.contentJson}
          initialText={note.contentText}
          onChange={onBody}
        />
      </div>
    </div>
  );
}
