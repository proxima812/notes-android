import { Archive, ArchiveRestore, Trash2 } from "lucide-react";

import type { NoteSummary } from "@/features/notes/api";
import { findGradient } from "@/shared/lib/gradients";

interface NoteCardProps {
  readonly note: NoteSummary;
  readonly onOpen: () => void;
  readonly onArchive: () => void;
  readonly onDelete: () => void;
  readonly busy: boolean;
}

export function NoteCard({
  note,
  onOpen,
  onArchive,
  onDelete,
  busy,
}: NoteCardProps): React.JSX.Element {
  const gradient = findGradient(note.color);
  const archived = note.isArchived;

  return (
    <li
      style={
        gradient === null
          ? undefined
          : { backgroundImage: gradient.surface, borderColor: gradient.border }
      }
      className={`flex items-start gap-2 rounded-2xl border p-4 ${
        gradient === null ? "bg-surface-raised border-border-subtle" : ""
      } ${busy ? "opacity-50" : ""}`}
    >
      <button
        type="button"
        onClick={onOpen}
        className="min-w-0 flex-1 text-left"
        aria-label={`Открыть заметку ${note.title === "" ? "без названия" : note.title}`}
      >
        <p className="truncate font-medium">
          {note.title === "" ? "Без названия" : note.title}
        </p>
        {note.preview !== "" && (
          <p
            className={`mt-1 line-clamp-2 text-sm whitespace-pre-line ${
              gradient === null ? "text-content-muted" : "text-content/75"
            }`}
          >
            {note.preview}
          </p>
        )}
      </button>

      <button
        type="button"
        aria-label={archived ? "Вернуть из архива" : "В архив"}
        onClick={onArchive}
        disabled={busy}
        className="text-content-muted hover:text-content -m-1 flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
      >
        {archived ? <ArchiveRestore className="size-5" /> : <Archive className="size-5" />}
      </button>

      <button
        type="button"
        aria-label="В корзину"
        onClick={onDelete}
        disabled={busy}
        className="text-content-muted hover:text-danger -m-1 flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
      >
        <Trash2 className="size-5" />
      </button>
    </li>
  );
}
