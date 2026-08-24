import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LayoutTemplate, Mic, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  archiveNote,
  createNote,
  listNotes,
  purgeExpiredTrash,
  purgeNote,
  restoreNote,
  trashNote,
  unarchiveNote,
  TRASH_RETENTION_MS,
  type NoteScope,
  type NoteSummary,
} from "@/features/notes/api";
import { buildDoc, buildText, type NoteTemplate } from "@/features/notes/templates";
import { NoteCard, type NoteCardActions } from "@/features/notes/ui/NoteCard";
import { listTags } from "@/features/organisation/api";
import type { QuickNote } from "@/features/quick-notes/api";
import { QuickNoteResult } from "@/features/quick-notes/ui/QuickNoteResult";
import { VoiceCapture } from "@/features/quick-notes/ui/VoiceCapture";
import { listNotesWithReminders } from "@/features/reminders/api";
import { TemplatePicker } from "@/features/notes/ui/TemplatePicker";
import { describeError } from "@/shared/api/errors";
import { useT, type StringKey } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import type { NoteId } from "@/shared/types/ids";
import { ACTION_BUTTON_FLOATING, ACTION_BUTTON_SECONDARY } from "@/shared/ui/actionButton";

const TABS = [
  { scope: "active", labelKey: "library.tabActive" },
  { scope: "archived", labelKey: "library.tabArchived" },
  { scope: "trash", labelKey: "library.tabTrash" },
] as const satisfies readonly { scope: NoteScope; labelKey: StringKey }[];

/** How often the trash re-reads the clock while it is on screen. */
const TRASH_TICK_MS = 20_000;

/**
 * One column of the masonry.
 *
 * Cards are dealt out one to each column in turn rather than measured: their
 * heights are not known until they are drawn, and dealing keeps the reading
 * order left to right, which is how a list of notes is read. The two columns end
 * up within a card's height of one another, which is what masonry is for.
 */
function columnOf(notes: readonly NoteSummary[], index: 0 | 1): readonly NoteSummary[] {
  return notes.filter((_, position) => position % 2 === index);
}

export function LibraryPage({
  onOpen,
  dictateOnOpen = false,
}: {
  readonly onOpen: (id: NoteId) => void;
  /** True when the launcher shortcut asked for dictation rather than the list. */
  readonly dictateOnOpen?: boolean;
}): React.JSX.Element {
  const client = useQueryClient();
  const t = useT();
  const [scope, setScope] = useState<NoteScope>("active");
  const [templatesOpen, setTemplatesOpen] = useState(false);

  // Dictation has two pieces of screen: the sheet while it listens, and the
  // line afterwards saying what was made. The line stays until it is dismissed
  // or another note is dictated — someone who looked away deserves to find out
  // what the app heard.
  const [dictating, setDictating] = useState(dictateOnOpen);
  const [dictated, setDictated] = useState<QuickNote | null>(null);

  // One tag at a time: narrowing by several at once would mostly produce empty
  // screens people cannot explain to themselves.
  const [filter, setFilter] = useState<string | null>(null);
  const tags = useQuery({ queryKey: ["tags"], queryFn: listTags });

  const notes = useQuery({
    queryKey: ["notes", scope, filter ?? ""],
    queryFn: () => listNotes({ scope, limit: 100, tagId: filter ?? undefined }),
  });

  // Every note's soonest firing, in one call rather than one per card. The
  // reminders screen already answers exactly this question, so the library asks
  // it the same way and reads only the first reminder of each note. Nothing in
  // the trash asks for anything, so there it is not asked at all.
  const due = useQuery({
    queryKey: ["notes", "with-reminders"],
    queryFn: () => listNotesWithReminders(),
    enabled: scope !== "trash",
  });

  const nextReminder = new Map<NoteId, number>(
    (due.data?.items ?? []).flatMap((entry) => {
      const soonest = entry.reminders[0];
      return soonest === undefined ? [] : [[entry.note.id, soonest.scheduledAt] as const];
    }),
  );

  // The badge has to be right even while the active tab is showing, so the
  // count is its own query rather than a read off `notes.data.total`. `limit: 1`
  // keeps it cheap: only the total is used.
  const archivedCount = useQuery({
    queryKey: ["notes", "archived", "count"],
    queryFn: async () => (await listNotes({ scope: "archived", limit: 1 })).total,
  });

  const refresh = (): void => {
    void client.invalidateQueries({ queryKey: ["notes"] });
    void client.invalidateQueries({ queryKey: ["app-info"] });
  };

  // The trash counts down, so while it is on screen the clock is read again
  // every so often: that is what moves "осталось 12 мин" along and what drops a
  // note off the screen the moment its hour is up. Nothing ticks on the other
  // two tabs, where no rendered value depends on the time.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (scope !== "trash") {
      return undefined;
    }

    setNow(Date.now());
    // Swept on arrival as well: the sweep is what actually erases, and the
    // screen about to be drawn should not be the one place still showing notes
    // the core no longer keeps.
    void purgeExpiredTrash().then(
      (removed) => {
        if (removed > 0) {
          refresh();
        }
      },
      () => {
        // A sweep that did not run leaves the trash a little fuller than it
        // should be, which is not worth an error on the screen.
      },
    );

    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, TRASH_TICK_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [scope]);

  const add = useMutation({
    mutationFn: () => createNote({}),
    onSuccess: (note) => {
      refresh();
      onOpen(note.id);
    },
  });

  // A template is an ordinary note that simply arrives pre-filled, so it goes
  // through the same create command and lands in the same editor.
  const addFromTemplate = useMutation({
    mutationFn: (template: NoteTemplate) =>
      createNote({
        noteType: template.noteType,
        title: template.title,
        contentJson: buildDoc(template),
        contentText: buildText(template),
      }),
    onSuccess: (note) => {
      setTemplatesOpen(false);
      refresh();
      onOpen(note.id);
    },
  });

  // One mutation per row would remount on every list refetch, so the row id is
  // carried in the variables and used to grey out just that card.
  const archive = useMutation({
    mutationFn: (input: { readonly id: NoteId; readonly archived: boolean }) =>
      input.archived ? unarchiveNote(input.id) : archiveNote(input.id),
    onSuccess: refresh,
  });

  const remove = useMutation({
    mutationFn: (id: NoteId) => trashNote(id),
    onSuccess: refresh,
  });

  const restore = useMutation({
    mutationFn: (id: NoteId) => restoreNote(id),
    onSuccess: refresh,
  });

  const purge = useMutation({
    mutationFn: (id: NoteId) => purgeNote(id),
    onSuccess: refresh,
  });

  useBackGuard(templatesOpen, () => {
    setTemplatesOpen(false);
  });

  let busyId: NoteId | null = null;
  if (archive.isPending) {
    busyId = archive.variables.id;
  } else if (remove.isPending) {
    busyId = remove.variables;
  } else if (restore.isPending) {
    busyId = restore.variables;
  } else if (purge.isPending) {
    busyId = purge.variables;
  }

  // A note whose hour is up is gone whether or not the sweep has run yet, so it
  // leaves the screen on the tick rather than on the next refetch.
  const listed = (notes.data?.items ?? []).filter(
    (note) =>
      scope !== "trash" || (note.deletedAt !== null && now - note.deletedAt < TRASH_RETENTION_MS),
  );

  const actionsFor = (note: NoteSummary): NoteCardActions => {
    if (scope === "trash") {
      const left = TRASH_RETENTION_MS - (now - (note.deletedAt ?? now));
      return {
        kind: "trash",
        minutesLeft: Math.floor(left / 60_000),
        onRestore: () => {
          restore.mutate(note.id);
        },
        onPurge: () => {
          purge.mutate(note.id);
        },
      };
    }

    return {
      kind: "library",
      onArchive: () => {
        archive.mutate({ id: note.id, archived: note.isArchived });
      },
      onDelete: () => {
        remove.mutate(note.id);
      },
    };
  };

  const renderCard = (note: NoteSummary): React.JSX.Element => (
    <NoteCard
      key={note.id}
      note={note}
      busy={busyId === note.id}
      actions={actionsFor(note)}
      reminderAt={nextReminder.get(note.id) ?? null}
      onOpen={() => {
        onOpen(note.id);
      }}
    />
  );

  let emptyKey: StringKey = "library.empty";
  if (scope === "archived") {
    emptyKey = "library.archiveEmpty";
  } else if (scope === "trash") {
    emptyKey = "library.trashEmpty";
  }

  const failure =
    archive.error ??
    remove.error ??
    restore.error ??
    purge.error ??
    add.error ??
    addFromTemplate.error;

  return (
    <div className="flex flex-col gap-4">
      {/* What these used to be: buttons floating over the bottom of the list.
          They sat on top of the notes and moved with nothing, so they are a row
          now, on the line the search field used to occupy. Creating is no longer
          among them — it is the one action worth a thumb of its own. */}
      <div className="flex items-center justify-end gap-2">
        <button
          type="button"
          aria-label={t("library.templates")}
          aria-expanded={templatesOpen}
          onClick={() => {
            setTemplatesOpen(true);
          }}
          className={ACTION_BUTTON_SECONDARY}
        >
          <LayoutTemplate className="size-5" />
        </button>

        {/* Dictation is the faster way in, not the only one: a phone in a quiet
            room is a phone that needs the keyboard. */}
        <button
          type="button"
          aria-label={t("quick.dictate")}
          onClick={() => {
            setDictating(true);
          }}
          className={ACTION_BUTTON_SECONDARY}
        >
          <Mic className="size-5" />
        </button>
      </div>

      {/* Directly under the buttons, before the list: a confirmation rendered
          after a hundred cards is a confirmation nobody ever sees. */}
      {dictated !== null && (
        <QuickNoteResult
          dictated={dictated}
          onOpen={onOpen}
          onChanged={refresh}
          onDismiss={() => {
            setDictated(null);
          }}
        />
      )}

      {dictating && (
        <VoiceCapture
          onCreated={(note) => {
            setDictating(false);
            setDictated(note);
            refresh();
          }}
          onClose={() => {
            setDictating(false);
          }}
        />
      )}

      {/* Filters only appear once there is something to filter by: an empty
          row of chips on a fresh install is furniture, not a feature. */}
      {(tags.data ?? []).length > 0 && (
        <div className="-mx-4 flex gap-2 overflow-x-auto px-4 pb-1">
          <button
            type="button"
            aria-pressed={filter === null}
            onClick={() => {
              setFilter(null);
            }}
            className={`min-h-11 shrink-0 rounded-xl px-3 text-sm ${
              filter === null
                ? "bg-accent text-accent-content"
                : "bg-surface-raised text-content-muted"
            }`}
          >
            {t("filing.all")}
          </button>
          {(tags.data ?? []).map((tag) => {
            const chosen = filter === tag.id;
            return (
              <button
                key={tag.id}
                type="button"
                aria-pressed={chosen}
                onClick={() => {
                  setFilter(chosen ? null : tag.id);
                }}
                className={`min-h-11 shrink-0 rounded-xl px-3 text-sm ${
                  chosen ? "bg-accent text-accent-content" : "bg-surface-raised text-content-muted"
                }`}
              >
                #{tag.name}
              </button>
            );
          })}
        </div>
      )}

      <div role="tablist" className="bg-surface-sunken flex gap-1 rounded-2xl p-1">
        {TABS.map((tab) => {
          const selected = scope === tab.scope;
          const count = tab.scope === "archived" ? (archivedCount.data ?? 0) : 0;
          // The trash is an icon and no word: it is the tab nobody goes looking
          // for, and the two that are named are the ones people came for.
          const isTrash = tab.scope === "trash";
          return (
            <button
              key={tab.scope}
              type="button"
              role="tab"
              aria-selected={selected}
              aria-label={isTrash ? t(tab.labelKey) : undefined}
              onClick={() => {
                setScope(tab.scope);
              }}
              className={`flex min-h-11 items-center justify-center gap-1.5 rounded-xl text-sm font-medium transition-colors ${
                isTrash ? "w-14 shrink-0" : "flex-1"
              } ${selected ? "bg-accent text-accent-content" : "text-content-muted"}`}
            >
              {isTrash ? <Trash2 className="size-5" /> : t(tab.labelKey)}
              {count > 0 && (
                <span
                  className={`rounded-full px-1.5 py-0.5 text-xs tabular-nums ${
                    selected ? "bg-accent-content/15" : "bg-surface-raised text-content-muted"
                  }`}
                >
                  {count}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Said once, at the top of the trash: the hour is the whole contract of
          this tab, and a card counting down only explains itself afterwards. */}
      {scope === "trash" && listed.length > 0 && (
        <p className="text-content-muted text-xs">{t("library.trashHint")}</p>
      )}

      {/* The list clears the floating button by its height plus a card's worth
          of air, so the last note is never the one hiding under it. */}
      <section className="pb-28">
        {notes.isPending && <p className="text-content-muted text-sm">{t("common.loading")}</p>}
        {notes.error !== null && (
          <p className="text-danger text-sm">{describeError(notes.error, t)}</p>
        )}
        {notes.data !== undefined && listed.length === 0 && (
          <p className="text-content-muted text-sm">{t(emptyKey)}</p>
        )}

        {/* Two columns of cards rather than one column of strips: a note is a
            piece of paper, and half the width is enough for one to say what it
            is. Each column is its own list so that a card can be as tall as it
            needs to be without the one beside it stretching to match. */}
        <div className="flex items-start gap-3">
          <ul className="flex min-w-0 flex-1 flex-col gap-3">
            {columnOf(listed, 0).map(renderCard)}
          </ul>
          <ul className="flex min-w-0 flex-1 flex-col gap-3">
            {columnOf(listed, 1).map(renderCard)}
          </ul>
        </div>
      </section>

      {failure !== null && <p className="text-danger text-sm">{describeError(failure, t)}</p>}

      {/* The one thing this screen is for, in the corner a thumb already rests
          in. Fixed to the window rather than to the list: it is as reachable at
          the top of a hundred notes as at the bottom of them. */}
      <button
        type="button"
        aria-label={t("library.newNote")}
        onClick={() => {
          add.mutate();
        }}
        disabled={add.isPending}
        className={`${ACTION_BUTTON_FLOATING} disabled:opacity-40`}
      >
        <Plus className="size-7" />
      </button>

      {templatesOpen && (
        <TemplatePicker
          busy={addFromTemplate.isPending}
          onPick={(template) => {
            addFromTemplate.mutate(template);
          }}
          onClose={() => {
            setTemplatesOpen(false);
          }}
        />
      )}
    </div>
  );
}
