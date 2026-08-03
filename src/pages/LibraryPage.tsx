import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LayoutTemplate, Plus, Search } from "lucide-react";
import { useState } from "react";

import {
  archiveNote,
  createNote,
  listNotes,
  search,
  splitHighlights,
  trashNote,
  unarchiveNote,
  type NoteScope,
  type SearchHit,
} from "@/features/notes/api";
import { buildDoc, buildText, type NoteTemplate } from "@/features/notes/templates";
import { NoteCard } from "@/features/notes/ui/NoteCard";
import { listFolders, listTags } from "@/features/organisation/api";
import { TemplatePicker } from "@/features/notes/ui/TemplatePicker";
import { ThemeSwitcher } from "@/features/settings/ui/ThemeSwitcher";
import { describeError } from "@/shared/api/errors";
import { useT, type StringKey } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import type { NoteId } from "@/shared/types/ids";
import {
  FLOATING_BUTTON_PRIMARY,
  FLOATING_BUTTON_SECONDARY,
  FLOATING_RIGHT_PRIMARY,
  FLOATING_RIGHT_THIRD,
} from "@/shared/ui/floatingButton";

const TABS = [
  { scope: "active", labelKey: "library.tabActive" },
  { scope: "archived", labelKey: "library.tabArchived" },
] as const satisfies readonly { scope: NoteScope; labelKey: StringKey }[];

function SearchResult({ hit }: { readonly hit: SearchHit }): React.JSX.Element {
  const t = useT();

  return (
    <li className="bg-surface-raised border-border-subtle rounded-2xl border p-4">
      <p className="truncate font-medium">{hit.title === "" ? t("common.untitled") : hit.title}</p>
      <p className="text-content-muted mt-1 text-sm">
        {splitHighlights(hit.snippet).map((part, index) =>
          part.highlighted ? (
            // eslint-disable-next-line react/no-array-index-key -- runs are positional
            <mark key={index} className="bg-accent/25 text-content rounded px-0.5">
              {part.text}
            </mark>
          ) : (
            <span key={index}>{part.text}</span>
          ),
        )}
      </p>
    </li>
  );
}

export function LibraryPage({
  onOpen,
}: {
  readonly onOpen: (id: NoteId) => void;
}): React.JSX.Element {
  const client = useQueryClient();
  const t = useT();
  const [scope, setScope] = useState<NoteScope>("active");
  const [query, setQuery] = useState("");
  const [templatesOpen, setTemplatesOpen] = useState(false);

  // One filter at a time: a note lives in folders and wears tags, and letting
  // both narrow at once would mostly produce empty screens people cannot
  // explain to themselves.
  const [filter, setFilter] = useState<{ kind: "folder" | "tag"; id: string } | null>(null);
  const folders = useQuery({ queryKey: ["folders"], queryFn: listFolders });
  const tags = useQuery({ queryKey: ["tags"], queryFn: listTags });

  const notes = useQuery({
    queryKey: ["notes", scope, filter?.kind ?? "", filter?.id ?? ""],
    queryFn: () =>
      listNotes({
        scope,
        limit: 100,
        folderId: filter?.kind === "folder" ? filter.id : undefined,
        tagId: filter?.kind === "tag" ? filter.id : undefined,
      }),
  });

  // The badge has to be right even while the active tab is showing, so the
  // count is its own query rather than a read off `notes.data.total`. `limit: 1`
  // keeps it cheap: only the total is used.
  const archivedCount = useQuery({
    queryKey: ["notes", "archived", "count"],
    queryFn: async () => (await listNotes({ scope: "archived", limit: 1 })).total,
  });

  const results = useQuery({
    queryKey: ["search", query],
    queryFn: () => search({ text: query, limit: 30, includeArchived: true }),
    enabled: query.trim().length > 0,
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

  const searching = query.trim().length > 0;

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
      <label className="bg-surface-sunken border-border-subtle flex items-center gap-2 rounded-2xl border px-4">
        <Search className="text-content-muted size-4 shrink-0" />
        <input
          type="search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          placeholder={t("library.search")}
          className="min-h-12 w-full bg-transparent outline-none"
        />
      </label>

      {searching ? (
        <section>
          {results.isPending && (
            <p className="text-content-muted text-sm">{t("library.searchInProgress")}</p>
          )}
          {results.error !== null && (
            <p className="text-danger text-sm">{describeError(results.error, t)}</p>
          )}
          {results.data !== undefined && results.data.items.length === 0 && (
            <p className="text-content-muted text-sm">{t("library.nothingFound")}</p>
          )}
          <ul className="flex flex-col gap-2">
            {results.data?.items.map((hit) => <SearchResult key={hit.id} hit={hit} />)}
          </ul>
        </section>
      ) : (
        <>
          {/* Filters only appear once there is something to filter by: an empty
              row of chips on a fresh install is furniture, not a feature. */}
          {((folders.data ?? []).length > 0 || (tags.data ?? []).length > 0) && (
            <div className="-mx-4 flex gap-2 overflow-x-auto px-4 pb-1">
              <button
                type="button"
                aria-pressed={filter === null}
                onClick={() => {
                  setFilter(null);
                }}
                className={`min-h-11 shrink-0 rounded-xl border px-3 text-sm ${
                  filter === null
                    ? "border-accent text-content"
                    : "border-border-subtle text-content-muted"
                }`}
              >
                {t("filing.all")}
              </button>
              {(folders.data ?? []).map((folder) => {
                const chosen = filter?.kind === "folder" && filter.id === folder.id;
                return (
                  <button
                    key={folder.id}
                    type="button"
                    aria-pressed={chosen}
                    onClick={() => {
                      setFilter(chosen ? null : { kind: "folder", id: folder.id });
                    }}
                    className={`min-h-11 shrink-0 rounded-xl border px-3 text-sm ${
                      chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
                    }`}
                  >
                    {folder.name}
                  </button>
                );
              })}
              {(tags.data ?? []).map((tag) => {
                const chosen = filter?.kind === "tag" && filter.id === tag.id;
                return (
                  <button
                    key={tag.id}
                    type="button"
                    aria-pressed={chosen}
                    onClick={() => {
                      setFilter(chosen ? null : { kind: "tag", id: tag.id });
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
        </>
      )}

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

      <button
        type="button"
        aria-label={t("library.templates")}
        aria-expanded={templatesOpen}
        onClick={() => {
          setTemplatesOpen(true);
        }}
        className={`${FLOATING_BUTTON_SECONDARY} ${FLOATING_RIGHT_THIRD}`}
      >
        <LayoutTemplate className="size-5" />
      </button>

      <ThemeSwitcher />

      <button
        type="button"
        aria-label={t("library.newNote")}
        onClick={() => {
          add.mutate();
        }}
        disabled={add.isPending}
        className={`${FLOATING_BUTTON_PRIMARY} ${FLOATING_RIGHT_PRIMARY} disabled:opacity-40`}
      >
        <Plus className="size-5" />
      </button>
    </div>
  );
}
