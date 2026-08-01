import {
  QueryClient,
  QueryClientProvider,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Plus, Search, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";

import {
  appInfo,
  createNote,
  listNotes,
  search,
  splitHighlights,
  trashNote,
  type NoteSummary,
  type SearchHit,
} from "@/features/notes/api";
import { describeError } from "@/shared/api/errors";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Everything is local; refetching on focus costs battery and buys nothing.
      refetchOnWindowFocus: false,
      staleTime: 10_000,
      retry: false,
    },
  },
});

function Diagnostics(): React.JSX.Element {
  const { data, error, isPending } = useQuery({
    queryKey: ["app-info"],
    queryFn: appInfo,
  });

  if (isPending) {
    return <p className="text-content-muted text-sm">Подключение к ядру…</p>;
  }
  if (error !== null) {
    return <p className="text-danger text-sm">{describeError(error)}</p>;
  }

  return (
    <p className="text-content-muted text-sm">
      Ядро {data.version} · схема v{data.schemaVersion} · {data.platform} ·{" "}
      {data.noteCount} заметок
    </p>
  );
}

function NoteRow({ note }: { readonly note: NoteSummary }): React.JSX.Element {
  const client = useQueryClient();
  const remove = useMutation({
    mutationFn: trashNote,
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["notes"] });
      void client.invalidateQueries({ queryKey: ["app-info"] });
    },
  });

  return (
    <li className="bg-surface-raised border-border-subtle flex items-start gap-3 rounded-2xl border p-4">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{note.title === "" ? "Без названия" : note.title}</p>
        {note.preview !== "" && (
          <p className="text-content-muted mt-1 line-clamp-2 text-sm">{note.preview}</p>
        )}
      </div>
      <button
        type="button"
        aria-label="В корзину"
        onClick={() => {
          remove.mutate(note.id);
        }}
        disabled={remove.isPending}
        className="text-content-muted hover:text-danger -m-2 flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
      >
        <Trash2 className="size-5" />
      </button>
    </li>
  );
}

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

function Library(): React.JSX.Element {
  const client = useQueryClient();
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");

  const notes = useQuery({
    queryKey: ["notes", "active"],
    queryFn: () => listNotes({ scope: "active", limit: 50 }),
  });

  const results = useQuery({
    queryKey: ["search", query],
    queryFn: () => search({ text: query, limit: 30 }),
    enabled: query.trim().length > 0,
  });

  const add = useMutation({
    mutationFn: createNote,
    onSuccess: () => {
      setDraft("");
      void client.invalidateQueries({ queryKey: ["notes"] });
      void client.invalidateQueries({ queryKey: ["app-info"] });
    },
  });

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const text = draft.trim();
    if (text.length === 0) {
      return;
    }
    add.mutate({ contentText: text });
  };

  const searching = query.trim().length > 0;

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

      <form onSubmit={submit} className="flex gap-2">
        <input
          type="text"
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          placeholder="Новая заметка"
          className="bg-surface-raised border-border-subtle min-h-12 w-full rounded-2xl border px-4 outline-none"
        />
        <button
          type="submit"
          aria-label="Создать"
          disabled={add.isPending || draft.trim().length === 0}
          className="bg-accent text-accent-content flex size-12 shrink-0 items-center justify-center rounded-2xl disabled:opacity-40"
        >
          <Plus className="size-5" />
        </button>
      </form>

      {add.error !== null && <p className="text-danger text-sm">{describeError(add.error)}</p>}

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
        <section>
          {notes.isPending && <p className="text-content-muted text-sm">Загрузка…</p>}
          {notes.error !== null && (
            <p className="text-danger text-sm">{describeError(notes.error)}</p>
          )}
          {notes.data !== undefined && notes.data.items.length === 0 && (
            <p className="text-content-muted text-sm">Пока пусто. Создайте первую заметку.</p>
          )}
          <ul className="flex flex-col gap-2">
            {notes.data?.items.map((note) => <NoteRow key={note.id} note={note} />)}
          </ul>
        </section>
      )}
    </div>
  );
}

export function App(): React.JSX.Element {
  document.documentElement.dataset["theme"] = "dark";

  return (
    <QueryClientProvider client={queryClient}>
      <main className="mx-auto flex min-h-dvh max-w-md flex-col gap-5 p-4">
        <header>
          <h1 className="text-2xl font-semibold tracking-tight">Заметки</h1>
          <Diagnostics />
        </header>
        <Library />
      </main>
    </QueryClientProvider>
  );
}
