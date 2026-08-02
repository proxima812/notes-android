import { Archive, ArchiveRestore, Trash2 } from "lucide-react";

import type { NoteSummary } from "@/features/notes/api";
import { useT } from "@/shared/i18n";
import { findGradient } from "@/shared/lib/gradients";

import { useCardSwipe } from "./useCardSwipe";

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
  const t = useT();
  const gradient = findGradient(note.color);
  const archived = note.isArchived;
  const title = note.title === "" ? t("common.untitled") : note.title;

  // Left is the reversible action and right is the destructive one, matching
  // where each icon is uncovered as the card slides away.
  const swipe = useCardSwipe({
    onSwipeLeft: onArchive,
    onSwipeRight: onDelete,
    disabled: busy,
  });

  const toDelete = swipe.dx > 0;
  const ArchiveIcon = archived ? ArchiveRestore : Archive;

  return (
    <li className="relative overflow-hidden rounded-2xl">
      {/* The action layer sits under the card and is revealed by the swipe, so
          the icon appears exactly in the space the card has vacated. It is
          hidden from assistive tech: both actions are also plain buttons. */}
      <div
        aria-hidden="true"
        className="bg-surface-sunken absolute inset-0 flex items-center justify-between px-6"
      >
        <span
          className={`transition-colors ${swipe.armed && toDelete ? "text-danger" : "text-content-muted"}`}
          style={{
            opacity: toDelete ? swipe.progress : 0,
            transform: `scale(${swipe.armed && toDelete ? 1.15 : 1})`,
          }}
        >
          <Trash2 className="size-5" />
        </span>
        <span
          className={`transition-colors ${swipe.armed && !toDelete ? "text-content" : "text-content-muted"}`}
          style={{
            opacity: toDelete ? 0 : swipe.progress,
            transform: `scale(${swipe.armed && !toDelete ? 1.15 : 1})`,
          }}
        >
          <ArchiveIcon className="size-5" />
        </span>
      </div>

      <div
        {...swipe.handlers}
        // The colour is named, not painted: `note-surface` reads the theme's
        // envelope in CSS, so a card restyles when the theme changes without
        // this component re-rendering.
        data-note={gradient === null ? undefined : gradient.id}
        style={{
          transform: `translateX(${String(swipe.dx)}px)`,
          // The browser keeps vertical panning; only sideways movement is ours.
          touchAction: "pan-y",
        }}
        className={`relative flex items-start gap-2 rounded-2xl border p-4 ${
          gradient === null ? "bg-surface-raised border-border-subtle" : "note-surface"
        } ${busy ? "opacity-50" : ""} ${swipe.dragging ? "" : "transition-transform duration-[180ms]"}`}
      >
        <button
          type="button"
          onClick={onOpen}
          className="min-w-0 flex-1 text-left"
          aria-label={t("card.open", { title })}
        >
          <p className="truncate font-medium">
            {title}
          </p>
          {note.preview !== "" && (
            <p
              className={`mt-1 line-clamp-2 text-sm whitespace-pre-line ${
                gradient === null ? "text-content-muted" : "note-ink-muted"
              }`}
            >
              {note.preview}
            </p>
          )}
        </button>

        <button
          type="button"
          aria-label={archived ? t("card.unarchive") : t("card.archive")}
          onClick={onArchive}
          disabled={busy}
          className="text-content-muted hover:text-content -m-1 flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
        >
          <ArchiveIcon className="size-5" />
        </button>

        <button
          type="button"
          aria-label={t("card.trash")}
          onClick={onDelete}
          disabled={busy}
          className="text-content-muted hover:text-danger -m-1 flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
        >
          <Trash2 className="size-5" />
        </button>
      </div>
    </li>
  );
}
