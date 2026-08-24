import { Archive, ArchiveRestore, BellRing, RotateCcw, Trash2 } from "lucide-react";

import type { NoteSummary } from "@/features/notes/api";
import { dueSentence } from "@/features/reminders/dueLabel";
import { useLanguage, useT } from "@/shared/i18n";
import { findGradient } from "@/shared/lib/gradients";

import { useCardSwipe } from "./useCardSwipe";

/**
 * What the two buttons at the foot of a card do.
 *
 * A card in the library is put away or thrown out; a card in the trash is taken
 * back or let go of. It is the same card and never both sets of actions, so the
 * pair travels as one value rather than as four optional callbacks.
 */
export type NoteCardActions =
  | {
      readonly kind: "library";
      readonly onArchive: () => void;
      readonly onDelete: () => void;
    }
  | {
      readonly kind: "trash";
      /** Whole minutes left of the hour, for the countdown on the card. */
      readonly minutesLeft: number;
      readonly onRestore: () => void;
      readonly onPurge: () => void;
    };

interface NoteCardProps {
  readonly note: NoteSummary;
  readonly onOpen: () => void;
  readonly actions: NoteCardActions;
  readonly busy: boolean;
  /**
   * The soonest firing still ahead on this note, or `null` for a note that is
   * not going to ask for anything.
   */
  readonly reminderAt?: number | null;
}

/**
 * The pill under the text: an icon and when it goes off, nothing else.
 *
 * `data-panel` is what hands the tint over to a coloured note — on one of those
 * the pill is the note's own ink thinned rather than a grey slab from the theme.
 * On a plain card the class is the whole of it.
 */
function ReminderChip({ at }: { readonly at: number }): React.JSX.Element {
  const t = useT();
  const [language] = useLanguage();

  return (
    <p
      data-panel=""
      className="bg-content/10 mt-2 inline-flex max-w-full items-center gap-1.5 self-start rounded-full py-1 pr-3 pl-2 text-xs font-medium"
    >
      <BellRing aria-label={t("reminder.title")} className="text-accent size-3.5 shrink-0" />
      <span className="truncate tabular-nums">{dueSentence(at, Date.now(), language, t)}</span>
    </p>
  );
}

/** The same pill, counting out what is left of the hour in the trash. */
function TrashChip({ minutesLeft }: { readonly minutesLeft: number }): React.JSX.Element {
  const t = useT();

  return (
    <p
      data-panel=""
      className="bg-content/10 mt-2 inline-flex max-w-full items-center gap-1.5 self-start rounded-full py-1 pr-3 pl-2 text-xs font-medium"
    >
      <Trash2 aria-hidden="true" className="size-3.5 shrink-0" />
      <span className="truncate tabular-nums">
        {minutesLeft < 1 ? t("card.trashSoon") : t("card.trashLeft", { minutes: minutesLeft })}
      </span>
    </p>
  );
}

export function NoteCard({
  note,
  onOpen,
  actions,
  busy,
  reminderAt = null,
}: NoteCardProps): React.JSX.Element {
  const t = useT();
  const gradient = findGradient(note.color);
  const archived = note.isArchived;
  const title = note.title === "" ? t("common.untitled") : note.title;
  const inTrash = actions.kind === "trash";

  // Left is the reversible action and right is the destructive one, matching
  // where each icon is uncovered as the card slides away. A card in the trash
  // does not slide at all: erasing for good is not something a thumb should be
  // able to do by accident, and the note is already deleted, so the gesture has
  // nothing reversible left to offer.
  const swipe = useCardSwipe({
    onSwipeLeft: () => {
      if (actions.kind === "library") {
        actions.onArchive();
      }
    },
    onSwipeRight: () => {
      if (actions.kind === "library") {
        actions.onDelete();
      }
    },
    disabled: busy || inTrash,
  });

  const toDelete = swipe.dx > 0;
  const ArchiveIcon = archived ? ArchiveRestore : Archive;

  return (
    <li className="relative overflow-hidden rounded-2xl">
      {/* The action layer sits under the card and is revealed by the swipe, so
          the icon appears exactly in the space the card has vacated. It is
          hidden from assistive tech: both actions are also plain buttons. */}
      {!inTrash && (
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
      )}

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
        // A column, not a row: two of these sit side by side now, and at half the
        // width there is no room for buttons beside the text. They go under it,
        // where the thumb reaches them just as easily.
        className={`relative flex flex-col rounded-2xl p-3 ${
          gradient === null ? "bg-surface-raised" : "note-surface"
        } ${busy ? "opacity-50" : ""} ${swipe.dragging ? "" : "transition-transform duration-[180ms]"}`}
      >
        <button
          type="button"
          onClick={onOpen}
          className="flex min-w-0 flex-col text-left"
          aria-label={t("card.open", { title })}
        >
          {/* Two lines rather than one: a title cut off at half the screen width
              usually stops before it has said anything. */}
          <p className="line-clamp-2 font-medium break-words">{title}</p>
          {note.preview !== "" && (
            <p
              className={`mt-1 line-clamp-4 text-sm break-words whitespace-pre-line ${
                gradient === null ? "text-content-muted" : "note-ink-muted"
              }`}
            >
              {note.preview}
            </p>
          )}
          {/* Under the preview and smaller than it: on a card the tags say
              where the note belongs, which is worth less room than what it
              says. One line — the rest are counted rather than wrapped. */}
          {note.tags.length > 0 && (
            <p
              className={`mt-2 truncate text-xs ${
                gradient === null ? "text-content-muted" : "note-ink-muted"
              }`}
            >
              {note.tags.map((tag) => `#${tag}`).join(" ")}
            </p>
          )}
        </button>

        {actions.kind === "trash" ? (
          <TrashChip minutesLeft={actions.minutesLeft} />
        ) : (
          reminderAt !== null && <ReminderChip at={reminderAt} />
        )}

        <div className="mt-1 -mb-1 flex justify-end gap-1">
          {actions.kind === "library" ? (
            <>
              <button
                type="button"
                aria-label={archived ? t("card.unarchive") : t("card.archive")}
                onClick={actions.onArchive}
                disabled={busy}
                className="text-content-muted hover:text-content flex size-9 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
              >
                <ArchiveIcon className="size-4" />
              </button>
              <button
                type="button"
                aria-label={t("card.trash")}
                onClick={actions.onDelete}
                disabled={busy}
                className="text-content-muted hover:text-danger flex size-9 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
              >
                <Trash2 className="size-4" />
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                aria-label={t("card.restore")}
                onClick={actions.onRestore}
                disabled={busy}
                className="text-content hover:text-accent flex size-9 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
              >
                <RotateCcw className="size-4" />
              </button>
              <button
                type="button"
                aria-label={t("card.purge")}
                onClick={actions.onPurge}
                disabled={busy}
                className="text-content-muted hover:text-danger flex size-9 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
              >
                <Trash2 className="size-4" />
              </button>
            </>
          )}
        </div>
      </div>
    </li>
  );
}
