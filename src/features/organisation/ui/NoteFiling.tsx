import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Plus } from "lucide-react";
import { useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import { ensureTag, listTags, setNoteTags, tagsOfNote } from "../api";

/**
 * Where a note is filed: its tags.
 *
 * Each is a chip that toggles. Picking is the common act — making a new tag is
 * the rarer one, so it sits behind the field at the end rather than in front of
 * the list.
 */
export function NoteFiling({ noteId }: { readonly noteId: NoteId }): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [newTag, setNewTag] = useState("");

  const allTags = useQuery({ queryKey: ["tags"], queryFn: listTags });
  const noteTags = useQuery({ queryKey: ["note-tags", noteId], queryFn: () => tagsOfNote(noteId) });

  const refresh = async (): Promise<void> => {
    // The counts on every chip move when a note is filed or unfiled.
    await Promise.all([
      client.invalidateQueries({ queryKey: ["tags"] }),
      client.invalidateQueries({ queryKey: ["notes"] }),
    ]);
  };

  const saveTags = useMutation({
    mutationFn: (ids: readonly string[]) => setNoteTags(noteId, ids),
    onSuccess: async (next) => {
      client.setQueryData(["note-tags", noteId], next);
      await refresh();
    },
  });

  const addTag = useMutation({
    mutationFn: ensureTag,
    onSuccess: async (tag) => {
      setNewTag("");
      await client.invalidateQueries({ queryKey: ["tags"] });
      saveTags.mutate([...(noteTags.data ?? []).map((item) => item.id), tag.id]);
    },
  });

  const error = saveTags.error ?? addTag.error;
  const busy = saveTags.isPending;

  const toggle = (ids: readonly string[], id: string): string[] =>
    ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];

  const chosenTags = (noteTags.data ?? []).map((item) => item.id);

  return (
    <section
      aria-label={t("filing.title")}
      data-panel=""
      className="bg-surface-sunken border-border-subtle flex flex-col gap-2 rounded-2xl border p-4"
    >
      <h3 className="text-content-muted text-sm font-medium">{t("filing.title")}</h3>
      <div className="flex flex-wrap gap-2">
        {(allTags.data ?? []).map((tag) => {
          const chosen = chosenTags.includes(tag.id);
          return (
            <button
              key={tag.id}
              type="button"
              aria-pressed={chosen}
              disabled={busy}
              onClick={() => {
                saveTags.mutate(toggle(chosenTags, tag.id));
              }}
              className={`flex min-h-11 items-center gap-2 rounded-xl border px-3 text-sm disabled:opacity-40 ${
                chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
              }`}
            >
              {chosen && <Check className="size-4" />}
              {tag.name}
            </button>
          );
        })}
      </div>
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={newTag}
          aria-label={t("filing.newTag")}
          placeholder={t("filing.newTag")}
          onChange={(event) => {
            setNewTag(event.target.value);
          }}
          className="bg-surface border-border-subtle text-content min-h-11 flex-1 rounded-xl border px-3 outline-none focus:border-accent"
        />
        <button
          type="button"
          aria-label={t("filing.addTag")}
          disabled={newTag.trim() === "" || addTag.isPending}
          onClick={() => {
            addTag.mutate(newTag);
          }}
          className="bg-surface-raised border-border-subtle text-content flex size-11 shrink-0 items-center justify-center rounded-xl border disabled:opacity-40"
        >
          <Plus className="size-4" />
        </button>
      </div>

      {error != null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(error, t)}
        </p>
      )}
    </section>
  );
}
