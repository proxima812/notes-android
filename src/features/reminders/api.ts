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

/**
 * Preset times, always `HH:MM` so they drop straight into `<input type="time">`.
 *
 * The core sorts and deduplicates, so the order here is the order to show.
 */
export const timePresetsSchema = z.array(z.string().regex(/^([01]\d|2[0-3]):[0-5]\d$/));

export type TimePresets = z.infer<typeof timePresetsSchema>;

export async function listReminderTimePresets(): Promise<TimePresets> {
  return callCommand("reminder_time_presets_list", timePresetsSchema);
}

/**
 * Stores the whole set, not a change to it.
 *
 * Adding one of the user's own times and deleting one of the six we ship are the
 * same call: this is the list now.
 */
export async function saveReminderTimePresets(presets: readonly string[]): Promise<TimePresets> {
  return callCommand("reminder_time_presets_save", timePresetsSchema, {
    presets: [...presets],
  });
}
