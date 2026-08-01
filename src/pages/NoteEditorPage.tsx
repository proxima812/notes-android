import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Palette } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { getNote, updateNote, type UpdateNoteRequest } from "@/features/notes/api";
import { RichTextEditor, type EditorSnapshot } from "@/features/notes/editor/RichTextEditor";
import { GradientPicker } from "@/features/notes/ui/GradientPicker";
import { describeError } from "@/shared/api/errors";
import { findGradient } from "@/shared/lib/gradients";
import type { NoteId } from "@/shared/types/ids";

/** Idle time before an edit is persisted. Long enough to coalesce typing. */
const AUTOSAVE_DELAY_MS = 600;

interface NoteEditorPageProps {
  readonly id: NoteId;
  readonly onBack: () => void;
}

export function NoteEditorPage({ id, onBack }: NoteEditorPageProps): React.JSX.Element {
  const client = useQueryClient();
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

  if (note.isPending) {
    return <p className="text-content-muted p-4 text-sm">Загрузка…</p>;
  }
  if (note.error !== null) {
    return <p className="text-danger p-4 text-sm">{describeError(note.error)}</p>;
  }
  if (note.data === null) {
    return <p className="text-content-muted p-4 text-sm">Заметка не найдена.</p>;
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
  const [title, setTitle] = useState(note.title);
  const [color, setColor] = useState(note.color);
  const [showColors, setShowColors] = useState(false);

  const gradient = findGradient(color);

  const onBody = useCallback(
    (snapshot: EditorSnapshot): void => {
      onPatch({ contentText: snapshot.contentText, contentJson: snapshot.contentJson });
    },
    [onPatch],
  );

  return (
    <div
      style={gradient === null ? undefined : { backgroundImage: gradient.surface }}
      className="flex min-h-dvh flex-col"
    >
      <header className="flex items-center gap-1 p-2">
        <button
          type="button"
          aria-label="Назад"
          onClick={onLeave}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <ArrowLeft className="size-5" />
        </button>

        <span className="text-content-muted flex-1 text-center text-xs">
          {saving ? "Сохранение…" : "Сохранено"}
        </span>

        <button
          type="button"
          aria-label="Цвет заметки"
          aria-pressed={showColors}
          onClick={() => {
            setShowColors((open) => !open);
          }}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <Palette className="size-5" />
        </button>
      </header>

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
          placeholder="Заголовок"
          aria-label="Заголовок"
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
