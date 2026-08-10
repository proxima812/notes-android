import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LayoutTemplate, Mic, Plus } from "lucide-react";
import { useState } from "react";

import {
  archiveNote,
  createNote,
  listNotes,
  trashNote,
  unarchiveNote,
  type NoteScope,
} from "@/features/notes/api";
import { buildDoc, buildText, type NoteTemplate } from "@/features/notes/templates";
import { NoteCard } from "@/features/notes/ui/NoteCard";
import { listTags } from "@/features/organisation/api";
import type { QuickNote } from "@/features/quick-notes/api";
import { QuickNoteResult } from "@/features/quick-notes/ui/QuickNoteResult";
import { VoiceCapture } from "@/features/quick-notes/ui/VoiceCapture";
import { TemplatePicker } from "@/features/notes/ui/TemplatePicker";
import { ThemeSwitcher } from "@/features/settings/ui/ThemeSwitcher";
import { describeError } from "@/shared/api/errors";
import { useT, type StringKey } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import type { NoteId } from "@/shared/types/ids";
import { ACTION_BUTTON_PRIMARY, ACTION_BUTTON_SECONDARY } from "@/shared/ui/actionButton";

const TABS = [
  { scope: "active", labelKey: "library.tabActive" },
  { scope: "archived", labelKey: "library.tabArchived" },
] as const satisfies readonly { scope: NoteScope; labelKey: StringKey }[];

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

  useBackGuard(templatesOpen, () => {
    setTemplatesOpen(false);
  });

  let busyId: NoteId | null = null;
  if (archive.isPending) {
    busyId = archive.variables.id;
  } else if (remove.isPending) {
    busyId = remove.variables;
  }

  return (
    <div className="flex flex-col gap-4">
      {/* What the three used to be: buttons floating over the bottom of the
          list. They sat on top of the notes and moved with nothing, so they are
          a row now, on the line the search field used to occupy. */}
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

        <ThemeSwitcher />

        {/* Dictation sits beside the plus rather than replacing it: it is the
            faster way in, not the only one, and a phone in a quiet room is a
            phone that needs the keyboard. */}
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

        <button
          type="button"
          aria-label={t("library.newNote")}
          onClick={() => {
            add.mutate();
          }}
          disabled={add.isPending}
          className={`${ACTION_BUTTON_PRIMARY} disabled:opacity-40`}
        >
          <Plus className="size-5" />
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
            className={`min-h-11 shrink-0 rounded-xl border px-3 text-sm ${
              filter === null ? "border-accent text-content" : "border-border-subtle text-content-muted"
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
                className={`min-h-11 shrink-0 rounded-xl border px-3 text-sm ${
                  chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
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
          return (
            <button
              key={tab.scope}
              type="button"
              role="tab"
              aria-selected={selected}
              onClick={() => {
                setScope(tab.scope);
              }}
              className={`flex min-h-11 flex-1 items-center justify-center gap-1.5 rounded-xl text-sm font-medium transition-colors ${
                selected ? "bg-accent text-accent-content" : "text-content-muted"
              }`}
            >
              {t(tab.labelKey)}
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

      <section>
        {notes.isPending && <p className="text-content-muted text-sm">{t("common.loading")}</p>}
        {notes.error !== null && (
          <p className="text-danger text-sm">{describeError(notes.error, t)}</p>
        )}
        {notes.data !== undefined && notes.data.items.length === 0 && (
          <p className="text-content-muted text-sm">
            {scope === "archived" ? t("library.archiveEmpty") : t("library.empty")}
          </p>
        )}
        <ul className="flex flex-col gap-2">
          {notes.data?.items.map((note) => (
            <NoteCard
              key={note.id}
              note={note}
              busy={busyId === note.id}
              onOpen={() => {
                onOpen(note.id);
              }}
              onArchive={() => {
                archive.mutate({ id: note.id, archived: note.isArchived });
              }}
              onDelete={() => {
                remove.mutate(note.id);
              }}
            />
          ))}
        </ul>
      </section>

      {(archive.error ?? remove.error ?? add.error ?? addFromTemplate.error) !== null && (
        <p className="text-danger text-sm">
          {describeError(archive.error ?? remove.error ?? add.error ?? addFromTemplate.error, t)}
        </p>
      )}

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
