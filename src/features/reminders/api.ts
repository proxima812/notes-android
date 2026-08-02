import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import {
  noteId,
  reminderId,
  reminderOccurrenceId,
  type NoteId,
  type ReminderId,
  type ReminderOccurrenceId,
} from "@/shared/types/ids";

const invalidIdIssue = {
  code: "custom" as const,
  message: "Ядро вернуло некорректный id",
};

const brandedReminderId = z.string().transform((value, context): ReminderId => {
  try {
    return reminderId(value);
  } catch {
    context.addIssue(invalidIdIssue);
    return z.NEVER;
  }
});

const brandedNoteId = z.string().transform((value, context): NoteId => {
  try {
    return noteId(value);
  } catch {
    context.addIssue(invalidIdIssue);
    return z.NEVER;
  }
});

const brandedOccurrenceId = z
  .string()
  .transform((value, context): ReminderOccurrenceId => {
    try {
      return reminderOccurrenceId(value);
    } catch {
      context.addIssue(invalidIdIssue);
      return z.NEVER;
    }
  });

export const reminderSoundSchema = z.object({
  id: z.string().regex(/^[a-z0-9_]+$/),
  label: z.string().min(1),
});

export const reminderSoundCatalogSchema = z.object({
  defaultSoundId: z.string().min(1),
  items: z.array(reminderSoundSchema).min(1),
});

export const reminderSchema = z.object({
  id: brandedReminderId,
  noteId: brandedNoteId,
  occurrenceId: brandedOccurrenceId,
  title: z.string().min(1),
  body: z.string(),
  scheduledAt: z.number().int(),
  timezone: z.string().min(1),
  sound: z.string().min(1),
  effectiveSoundId: z.string().min(1),
  effectiveSoundLabel: z.string().min(1),
  isExact: z.boolean(),
});

export type Reminder = z.infer<typeof reminderSchema>;
export type ReminderSound = z.infer<typeof reminderSoundSchema>;
export type ReminderSoundCatalog = z.infer<typeof reminderSoundCatalogSchema>;

export interface UpsertReminderRequest {
  readonly noteId: NoteId;
  readonly title: string;
  readonly body: string;
  readonly scheduledAt: number;
  readonly timezone: string;
  readonly sound: string;
}

export async function getReminderForNote(noteIdValue: NoteId): Promise<Reminder | null> {
  return callCommand("reminders_get_for_note", reminderSchema.nullable(), {
    noteId: noteIdValue,
  });
}

export async function upsertReminderForNote(
  request: UpsertReminderRequest,
): Promise<Reminder> {
  return callCommand("reminders_upsert_for_note", reminderSchema, { request });
}

export async function deleteReminderForNote(noteIdValue: NoteId): Promise<null> {
  return callCommand("reminders_delete_for_note", z.null(), {
    noteId: noteIdValue,
  });
}

/**
 * Collects the note a notification tap asked to open.
 *
 * The core clears the target as it answers, so this returns a note once per tap
 * and `null` on every other call — which is why it is safe to ask on every
 * start and every return from the background.
 */
export async function takeReminderLaunchTarget(): Promise<NoteId | null> {
  return callCommand(
    "reminders_take_launch_target",
    brandedNoteId.nullable(),
  );
}

export async function listReminderSounds(): Promise<ReminderSoundCatalog> {
  return callCommand("reminder_sounds_list", reminderSoundCatalogSchema);
}
