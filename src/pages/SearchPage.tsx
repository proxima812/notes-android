import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Search } from "lucide-react";
import { useState } from "react";

import { search, splitHighlights, type SearchHit } from "@/features/notes/api";
import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import { noteId, type NoteId } from "@/shared/types/ids";

function SearchResult({
  hit,
  onOpen,
}: {
  readonly hit: SearchHit;
  readonly onOpen: (id: NoteId) => void;
}): React.JSX.Element {
  const t = useT();
  const title = hit.title === "" ? t("common.untitled") : hit.title;

  const body = (
    <>
      <p className="truncate font-medium">{title}</p>
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
    </>
  );

  const card = "bg-surface-raised rounded-2xl p-4";

  // Only a note hit carries a note id. A task or an attachment matched inside
  // some note, and this screen has nowhere to send those, so they are shown and
  // left flat rather than made to look tappable and then doing nothing.
  if (hit.entity !== "note") {
    return <li className={card}>{body}</li>;
  }

  return (
    <li>
      <button
        type="button"
        aria-label={t("card.open", { title })}
        onClick={() => {
          onOpen(noteId(hit.id));
        }}
        className={`${card} w-full text-left`}
      >
        {body}
      </button>
    </li>
  );
}

/**
 * Search, on a screen of its own.
 *
 * It used to be a field pinned above the library, which spent the first line of
 * the first screen on something most openings never touch and left the results
 * to replace the list underneath it. As a screen it can do what search wants:
 * the field is focused on arrival, the archive is included without a tab to
 * switch, and leaving puts the library back exactly as it was.
 */
export function SearchPage({
  onBack,
  onOpen,
}: {
  readonly onBack: () => void;
  readonly onOpen: (id: NoteId) => void;
}): React.JSX.Element {
  const t = useT();
  const [query, setQuery] = useState("");
  const asked = query.trim().length > 0;

  useBackGuard(true, onBack);

  const results = useQuery({
    queryKey: ["search", query],
    queryFn: () => search({ text: query, limit: 30, includeArchived: true }),
    enabled: asked,
  });

  return (
    <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-4 p-4">
      <header className="flex items-center gap-2">
        <button
          type="button"
          aria-label={t("common.back")}
          onClick={onBack}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <ArrowLeft className="size-5" />
        </button>
        <label className="bg-surface-sunken flex flex-1 items-center gap-2 rounded-2xl px-4">
          <Search className="text-content-muted size-4 shrink-0" />
          <input
            type="search"
            // The screen exists only to be typed into, so it opens with the
            // keyboard up rather than asking for one more tap to start.
            // eslint-disable-next-line jsx-a11y/no-autofocus -- the whole screen is this field
            autoFocus
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
            placeholder={t("library.search")}
            className="min-h-12 w-full bg-transparent outline-none"
          />
        </label>
      </header>

      <section>
        {!asked && <p className="text-content-muted text-sm">{t("library.searchHint")}</p>}
        {asked && results.isPending && (
          <p className="text-content-muted text-sm">{t("library.searchInProgress")}</p>
        )}
        {results.error !== null && (
          <p className="text-danger text-sm">{describeError(results.error, t)}</p>
        )}
        {results.data !== undefined && results.data.items.length === 0 && (
          <p className="text-content-muted text-sm">{t("library.nothingFound")}</p>
        )}
        <ul className="flex flex-col gap-2">
          {results.data?.items.map((hit) => (
            <SearchResult key={`${hit.entity}:${hit.id}`} hit={hit} onOpen={onOpen} />
          ))}
        </ul>
      </section>
    </main>
  );
}
