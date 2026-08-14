import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Plus, X } from "lucide-react";
import { useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import { deleteTag, ensureTag, listTags, setNoteTags, tagsOfNote } from "../api";

/**
 * Where a note is filed: its tags.
 *
 * Each is a chip that toggles. Picking is the common act — making a new tag is
 * the rarer one, so it sits behind the field at the end rather than in front of
 * the list.
 *
 * Deleting one is rarer still, and unlike un-picking it cannot be undone by
 * tapping again: the tag leaves every note wearing it. So it lives behind an
 * edit mode. That mode is the confirmation — a chip cannot be destroyed by the
 * same tap that would have selected it a moment earlier.
 */
export function NoteFiling({ noteId }: { readonly noteId: NoteId }): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [newTag, setNewTag] = useState("");
  const [editing, setEditing] = useState(false);

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

  const removeTag = useMutation({
    mutationFn: deleteTag,
    onSuccess: async () => {
      // The tag is gone from every note, so this note's own list is stale too.
      await Promise.all([
        client.invalidateQueries({ queryKey: ["note-tags"] }),
        refresh(),
      ]);
    },
  });

  const error = saveTags.error ?? addTag.error ?? removeTag.error;
  const busy = saveTags.isPending || removeTag.isPending;

  const toggle = (ids: readonly string[], id: string): string[] =>
    ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];

  const chosenTags = (noteTags.data ?? []).map((item) => item.id);

  return (
    <section
      aria-label={t("filing.title")}
      data-panel=""
      className="bg-surface-sunken border-border-subtle flex flex-col gap-2 rounded-2xl border p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-content-muted text-sm font-medium">{t("filing.title")}</h3>
        {(allTags.data ?? []).length > 0 && (
          <button
            type="button"
            aria-pressed={editing}
            onClick={() => {
              setEditing((on) => !on);
            }}
            className="text-content-muted min-h-11 px-1 text-sm underline-offset-4 hover:underline"
          >
            {editing ? t("filing.manageDone") : t("filing.manage")}
          </button>
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        {(allTags.data ?? []).map((tag) => {
          const chosen = chosenTags.includes(tag.id);
          const frame = `flex min-h-11 items-center gap-2 rounded-xl border px-3 text-sm ${
            chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
          }`;

          // Two controls cannot nest in one button, so the editing chip is a
          // frame holding the name and the × rather than a button wearing one.
          if (editing) {
            return (
              <span key={tag.id} className={frame}>
                {tag.name}
                <button
                  type="button"
                  aria-label={t("filing.removeTag", { name: tag.name })}
                  disabled={busy}
                  onClick={() => {
                    removeTag.mutate(tag.id);
                  }}
                  className="text-danger -mr-1 flex size-8 items-center justify-center rounded-lg disabled:opacity-40"
                >
                  <X className="size-4" />
                </button>
              </span>
            );
          }

          return (
            <button
              key={tag.id}
              type="button"
              aria-pressed={chosen}
              disabled={busy}
              onClick={() => {
                saveTags.mutate(toggle(chosenTags, tag.id));
              }}
              className={`${frame} disabled:opacity-40`}
            >
              {chosen && <Check className="size-4" />}
              {tag.name}
            </button>
          );
        })}
      </div>
      {/* Un-picking chips one by one is fine for one; someone who opened this
          panel by accident and tapped a few wants them all off at once. The
          tags themselves stay — this takes them off this note only, which is
          why it does not ask first: every chip is one tap from coming back. */}
      {chosenTags.length > 0 && !editing && (
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            saveTags.mutate([]);
          }}
          className="text-content-muted min-h-11 self-start px-1 text-sm underline-offset-4 hover:underline disabled:opacity-40"
        >
          {t("filing.clear", { count: chosenTags.length })}
        </button>
      )}

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
