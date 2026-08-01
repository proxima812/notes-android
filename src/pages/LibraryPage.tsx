import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Search } from "lucide-react";
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
import { NoteCard } from "@/features/notes/ui/NoteCard";
import { describeError } from "@/shared/api/errors";
import type { NoteId } from "@/shared/types/ids";

const TABS = [
  { scope: "active", label: "Заметки" },
  { scope: "archived", label: "Архив" },
] as const satisfies readonly { scope: NoteScope; label: string }[];

function SearchResult({ hit }: { readonly hit: SearchHit }): React.JSX.Element {
  return (
    <li className="bg-surface-raised border-border-subtle rounded-2xl border p-4">
      <p className="truncate font-medium">{hit.title === "" ? "Без названия" : hit.title}</p>
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
  const [scope, setScope] = useState<NoteScope>("active");
  const [query, setQuery] = useState("");

  const notes = useQuery({
    queryKey: ["notes", scope],
    queryFn: () => listNotes({ scope, limit: 100 }),
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
          placeholder="Поиск"
          className="min-h-12 w-full bg-transparent outline-none"
        />
      </label>

      {searching ? (
        <section>
          {results.isPending && <p className="text-content-muted text-sm">Поиск…</p>}
          {results.error !== null && (
            <p className="text-danger text-sm">{describeError(results.error)}</p>
          )}
          {results.data !== undefined && results.data.items.length === 0 && (
            <p className="text-content-muted text-sm">Ничего не найдено.</p>
          )}
          <ul className="flex flex-col gap-2">
            {results.data?.items.map((hit) => <SearchResult key={hit.id} hit={hit} />)}
          </ul>
        </section>
      ) : (
        <>
          <div role="tablist" className="bg-surface-sunken flex gap-1 rounded-2xl p-1">
            {TABS.map((tab) => (
              <button
                key={tab.scope}
                type="button"
                role="tab"
                aria-selected={scope === tab.scope}
                onClick={() => {
                  setScope(tab.scope);
                }}
                className={`min-h-11 flex-1 rounded-xl text-sm font-medium transition-colors ${
                  scope === tab.scope ? "bg-accent text-accent-content" : "text-content-muted"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <section>
            {notes.isPending && <p className="text-content-muted text-sm">Загрузка…</p>}
            {notes.error !== null && (
              <p className="text-danger text-sm">{describeError(notes.error)}</p>
            )}
            {notes.data !== undefined && notes.data.items.length === 0 && (
              <p className="text-content-muted text-sm">
                {scope === "archived" ? "Архив пуст." : "Пока пусто. Создайте первую заметку."}
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

      {(archive.error ?? remove.error ?? add.error) !== null && (
        <p className="text-danger text-sm">
          {describeError(archive.error ?? remove.error ?? add.error)}
        </p>
      )}

      <button
        type="button"
        aria-label="Новая заметка"
        onClick={() => {
          add.mutate();
        }}
        disabled={add.isPending}
        className="bg-accent text-accent-content fixed right-5 bottom-6 flex size-14 items-center justify-center rounded-2xl shadow-lg disabled:opacity-40"
      >
        <Plus className="size-6" />
      </button>
    </div>
  );
}
