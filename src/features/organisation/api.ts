import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import type { NoteId } from "@/shared/types/ids";

export const tagSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  /** Notes wearing it, so the useful ones can come first. */
  usageCount: z.number().int(),
});

export const folderSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  noteCount: z.number().int(),
});

export type Tag = z.infer<typeof tagSchema>;
export type Folder = z.infer<typeof folderSchema>;

export async function listTags(): Promise<Tag[]> {
  return callCommand("tags_list", z.array(tagSchema));
}

/**
 * Finds a tag by name or makes it.
 *
 * Names are matched ignoring case, so typing `Работа` when `работа` exists
 * reuses it rather than creating a second one that looks the same.
 */
export async function ensureTag(name: string): Promise<Tag> {
  return callCommand("tags_ensure", tagSchema, { name });
}

export async function deleteTag(id: string): Promise<null> {
  return callCommand("tags_delete", z.null(), { id });
}

export async function tagsOfNote(noteId: NoteId): Promise<Tag[]> {
  return callCommand("tags_of_note", z.array(tagSchema), { noteId });
}

/** Sends the whole set, not a change to it. */
export async function setNoteTags(noteId: NoteId, tags: readonly string[]): Promise<Tag[]> {
  return callCommand("tags_set_for_note", z.array(tagSchema), {
    noteId,
    tags: [...tags],
  });
}

export async function listFolders(): Promise<Folder[]> {
  return callCommand("folders_list", z.array(folderSchema));
}

export async function createFolder(name: string): Promise<Folder> {
  return callCommand("folders_create", folderSchema, { name });
}

export async function deleteFolder(id: string): Promise<null> {
  return callCommand("folders_delete", z.null(), { id });
}

export async function foldersOfNote(noteId: NoteId): Promise<Folder[]> {
  return callCommand("folders_of_note", z.array(folderSchema), { noteId });
}

export async function setNoteFolders(
  noteId: NoteId,
  folders: readonly string[],
): Promise<Folder[]> {
  return callCommand("folders_set_for_note", z.array(folderSchema), {
    noteId,
    folders: [...folders],
  });
}
