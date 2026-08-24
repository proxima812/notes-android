import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import { noteId, type NoteId } from "@/shared/types/ids";

/**
 * Schemas mirroring the Rust DTOs. These are the single place where the shape
 * of the bridge is asserted; screens consume the inferred types and never see
 * an `unknown`.
 */

export const noteTypeSchema = z.enum([
  "text",
  "rich_text",
  "checklist",
  "task_list",
  "journal",
  "daily_note",
  "meeting",
  "idea",
  "project",
  "contact",
  "shopping_list",
  "habit",
  "password_hint",
  "bookmark",
  "code_snippet",
  "voice_note",
  "drawing",
]);

export type NoteType = z.infer<typeof noteTypeSchema>;

const brandedNoteId = z.string().transform((value, context): NoteId => {
  try {
    return noteId(value);
  } catch {
    context.addIssue({ code: "custom", message: "Ядро вернуло некорректный id заметки" });
    return z.NEVER;
  }
});

export const noteSchema = z.object({
  id: brandedNoteId,
  noteType: noteTypeSchema,
  title: z.string(),
  contentText: z.string(),
  contentJson: z.string().nullable(),
  color: z.string().nullable(),
  background: z.string().nullable(),
  isPinned: z.boolean(),
  isFavorite: z.boolean(),
  isArchived: z.boolean(),
  isReadonly: z.boolean(),
  position: z.number().int(),
  wordCount: z.number().int(),
  charCount: z.number().int(),
  revision: z.number().int(),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
  deletedAt: z.number().int().nullable(),
});

export type Note = z.infer<typeof noteSchema>;

export const noteSummarySchema = z.object({
  id: brandedNoteId,
  noteType: noteTypeSchema,
  title: z.string(),
  preview: z.string(),
  color: z.string().nullable(),
  isPinned: z.boolean(),
  isFavorite: z.boolean(),
  isArchived: z.boolean(),
  wordCount: z.number().int(),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
  deletedAt: z.number().int().nullable(),
  /** Names only: a card shows its tags, it does not let you pick by them. */
  tags: z.array(z.string()),
});

export type NoteSummary = z.infer<typeof noteSummarySchema>;

function pageSchema<T extends z.ZodType>(item: T) {
  return z.object({
    items: z.array(item),
    total: z.number().int(),
    limit: z.number().int(),
    offset: z.number().int(),
    hasMore: z.boolean(),
  });
}

export type Page<T> = {
  readonly items: readonly T[];
  readonly total: number;
  readonly limit: number;
  readonly offset: number;
  readonly hasMore: boolean;
};

export const noteScopeSchema = z.enum([
  "active",
  "archived",
  "trash",
  "favorites",
  "all_except_trash",
]);

export type NoteScope = z.infer<typeof noteScopeSchema>;

export const noteSortSchema = z.enum([
  "pinned_then_updated",
  "updated_desc",
  "updated_asc",
  "created_desc",
  "created_asc",
  "title_asc",
  "title_desc",
  "manual",
  "next_reminder",
]);

export type NoteSort = z.infer<typeof noteSortSchema>;

export interface ListNotesRequest {
  readonly scope?: NoteScope;
  readonly sort?: NoteSort;
  readonly tagId?: string | undefined;
  readonly noteType?: NoteType;
  readonly pinnedOnly?: boolean;
  readonly limit?: number;
  readonly offset?: number;
}

export interface CreateNoteRequest {
  readonly noteType?: NoteType;
  readonly title?: string;
  readonly contentText?: string;
  readonly contentJson?: string;
  readonly color?: string;
}

/**
 * Every field is optional and an omitted one is left untouched. `null` is a
 * deliberate value the core stores as SQL NULL, which is how a colour is
 * cleared without the same call also wiping the body.
 */
export interface UpdateNoteRequest {
  readonly title?: string;
  readonly contentText?: string;
  readonly contentJson?: string | null;
  readonly color?: string | null;
  readonly isPinned?: boolean;
  readonly isFavorite?: boolean;
  readonly isArchived?: boolean;
  readonly isReadonly?: boolean;
}

export async function listNotes(request: ListNotesRequest = {}): Promise<Page<NoteSummary>> {
  return callCommand("notes_list", pageSchema(noteSummarySchema), { request });
}

export async function createNote(request: CreateNoteRequest = {}): Promise<Note> {
  return callCommand("notes_create", noteSchema, { request });
}

export async function getNote(id: NoteId): Promise<Note | null> {
  return callCommand("notes_get", noteSchema.nullable(), { id });
}

export async function updateNote(id: NoteId, request: UpdateNoteRequest): Promise<Note> {
  return callCommand("notes_update", noteSchema, { id, request });
}

export async function trashNote(id: NoteId): Promise<null> {
  return callCommand("notes_trash", z.null(), { id });
}

export async function restoreNote(id: NoteId): Promise<Note> {
  return callCommand("notes_restore", noteSchema, { id });
}

export async function purgeNote(id: NoteId): Promise<null> {
  return callCommand("notes_purge", z.null(), { id });
}

export async function emptyTrash(): Promise<number> {
  return callCommand("notes_empty_trash", z.number().int(), {});
}

/**
 * Lets go of everything whose hour in the trash is up, answering how many.
 *
 * Nothing runs on a timer inside the core, so this is asked when the answer is
 * about to be shown: on start and whenever the trash is opened. The screen also
 * hides an expired note it is still holding, because the sweep happens when
 * someone looks rather than on the minute.
 */
export async function purgeExpiredTrash(): Promise<number> {
  return callCommand("notes_purge_expired", z.number().int(), {});
}

/** How long a note stays in the trash. Mirrors `TRASH_RETENTION_MINUTES`. */
export const TRASH_RETENTION_MS = 60 * 60 * 1000;

export async function duplicateNote(id: NoteId): Promise<Note> {
  return callCommand("notes_duplicate", noteSchema, { id });
}

/** Done with, but kept for later reading. Distinct from the trash. */
export async function archiveNote(id: NoteId): Promise<Note> {
  return updateNote(id, { isArchived: true });
}

export async function unarchiveNote(id: NoteId): Promise<Note> {
  return updateNote(id, { isArchived: false });
}

export async function recolorNote(id: NoteId, color: string | null): Promise<Note> {
  return updateNote(id, { color });
}

// ------------------------------------------------------------- search --

export const searchEntitySchema = z.enum(["note", "task", "attachment"]);

export const searchHitSchema = z.object({
  entity: searchEntitySchema,
  id: z.string(),
  title: z.string(),
  snippet: z.string(),
  rank: z.number(),
});

export type SearchHit = z.infer<typeof searchHitSchema>;

export interface SearchRequest {
  readonly text: string;
  readonly includeArchived?: boolean;
  readonly includeTrashed?: boolean;
  readonly limit?: number;
  readonly offset?: number;
}

export async function search(request: SearchRequest): Promise<Page<SearchHit>> {
  return callCommand("search_run", pageSchema(searchHitSchema), { request });
}

export async function recentQueries(limit?: number): Promise<readonly string[]> {
  return callCommand("search_recent", z.array(z.string()), { limit: limit ?? null });
}

// --------------------------------------------------------- diagnostics --

export const appInfoSchema = z.object({
  name: z.string(),
  version: z.string(),
  platform: z.string(),
  schemaVersion: z.number().int(),
  noteCount: z.number().int(),
});

export type AppInfo = z.infer<typeof appInfoSchema>;

export async function appInfo(): Promise<AppInfo> {
  return callCommand("app_info", appInfoSchema);
}

/** Markers the Rust search engine wraps around matches inside a snippet. */
export const HIGHLIGHT_OPEN = "⁢[";
export const HIGHLIGHT_CLOSE = "]⁢";

/**
 * Splits a snippet into plain and highlighted runs so the UI can render matches
 * without ever inserting HTML from search results.
 */
export function splitHighlights(
  snippet: string,
): readonly { readonly text: string; readonly highlighted: boolean }[] {
  const parts: { text: string; highlighted: boolean }[] = [];
  let rest = snippet;

  while (rest.length > 0) {
    const open = rest.indexOf(HIGHLIGHT_OPEN);
    if (open === -1) {
      parts.push({ text: rest, highlighted: false });
      break;
    }

    if (open > 0) {
      parts.push({ text: rest.slice(0, open), highlighted: false });
    }

    const afterOpen = rest.slice(open + HIGHLIGHT_OPEN.length);
    const close = afterOpen.indexOf(HIGHLIGHT_CLOSE);
    if (close === -1) {
      parts.push({ text: afterOpen, highlighted: false });
      break;
    }

    parts.push({ text: afterOpen.slice(0, close), highlighted: true });
    rest = afterOpen.slice(close + HIGHLIGHT_CLOSE.length);
  }

  return parts.filter((part) => part.text.length > 0);
}
