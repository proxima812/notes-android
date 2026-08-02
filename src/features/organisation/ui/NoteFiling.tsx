import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, FolderPlus, Plus } from "lucide-react";
import { useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import {
  createFolder,
  ensureTag,
  foldersOfNote,
  listFolders,
  listTags,
  setNoteFolders,
  setNoteTags,
  tagsOfNote,
} from "../api";

/**
 * Where a note is filed: its folders and its tags.
 *
 * Both are chips that toggle. Picking is the common act — making a new label is
 * the rarer one, so it sits behind the field at the end rather than in front of
 * the list.
 */
export function NoteFiling({ noteId }: { readonly noteId: NoteId }): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [newTag, setNewTag] = useState("");
  const [newFolder, setNewFolder] = useState("");

  const allTags = useQuery({ queryKey: ["tags"], queryFn: listTags });
  const allFolders = useQuery({ queryKey: ["folders"], queryFn: listFolders });
  const noteTags = useQuery({ queryKey: ["note-tags", noteId], queryFn: () => tagsOfNote(noteId) });
  const noteFolders = useQuery({
    queryKey: ["note-folders", noteId],
    queryFn: () => foldersOfNote(noteId),
  });

  const refresh = async (): Promise<void> => {
    // The counts on every chip move when a note is filed or unfiled.
    await Promise.all([
      client.invalidateQueries({ queryKey: ["tags"] }),
      client.invalidateQueries({ queryKey: ["folders"] }),
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
  const saveFolders = useMutation({
    mutationFn: (ids: readonly string[]) => setNoteFolders(noteId, ids),
    onSuccess: async (next) => {
      client.setQueryData(["note-folders", noteId], next);
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
  const addFolder = useMutation({
    mutationFn: createFolder,
    onSuccess: async (folder) => {
      setNewFolder("");
      await client.invalidateQueries({ queryKey: ["folders"] });
      saveFolders.mutate([...(noteFolders.data ?? []).map((item) => item.id), folder.id]);
    },
  });

  const error =
    saveTags.error ?? saveFolders.error ?? addTag.error ?? addFolder.error;
  const busy = saveTags.isPending || saveFolders.isPending;

  const toggle = (ids: readonly string[], id: string): string[] =>
    ids.includes(id) ? ids.filter((item) => item !== id) : [...ids, id];

  const chosenTags = (noteTags.data ?? []).map((item) => item.id);
  const chosenFolders = (noteFolders.data ?? []).map((item) => item.id);

  return (
    <section
      aria-label={t("filing.title")}
      className="bg-surface-sunken border-border-subtle flex flex-col gap-4 rounded-2xl border p-4"
    >
      <div className="flex flex-col gap-2">
        <h3 className="text-content-muted text-sm font-medium">{t("filing.folders")}</h3>
        <div className="flex flex-wrap gap-2">
          {(allFolders.data ?? []).map((folder) => {
            const chosen = chosenFolders.includes(folder.id);
            return (
              <button
                key={folder.id}
                type="button"
                aria-pressed={chosen}
                disabled={busy}
                onClick={() => {
                  saveFolders.mutate(toggle(chosenFolders, folder.id));
                }}
                className={`flex min-h-11 items-center gap-2 rounded-xl border px-3 text-sm disabled:opacity-40 ${
                  chosen ? "border-accent text-content" : "border-border-subtle text-content-muted"
                }`}
              >
                {chosen && <Check className="size-4" />}
                {folder.name}
              </button>
            );
          })}
        </div>
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={newFolder}
            aria-label={t("filing.newFolder")}
            placeholder={t("filing.newFolder")}
            onChange={(event) => {
              setNewFolder(event.target.value);
            }}
            className="bg-surface border-border-subtle text-content min-h-11 flex-1 rounded-xl border px-3 outline-none focus:border-accent"
          />
          <button
            type="button"
            aria-label={t("filing.addFolder")}
            disabled={newFolder.trim() === "" || addFolder.isPending}
            onClick={() => {
              addFolder.mutate(newFolder);
            }}
            className="bg-surface-raised border-border-subtle text-content flex size-11 shrink-0 items-center justify-center rounded-xl border disabled:opacity-40"
          >
            <FolderPlus className="size-4" />
          </button>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        <h3 className="text-content-muted text-sm font-medium">{t("filing.tags")}</h3>
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
      </div>

      {error != null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(error, t)}
        </p>
      )}
    </section>
  );
}
