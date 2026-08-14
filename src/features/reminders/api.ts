import { z } from "zod";

import { noteSummarySchema, type Page } from "@/features/notes/api";
import { callCommand } from "@/shared/api/command";
import {
  deviceTimeZone,
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

export const reminderSoundKindSchema = z.enum(["preset", "system", "custom"]);

/**
 * One pickable sound. Preset ids are bare (`death_and_rebirth`); device and
 * user sounds carry their origin in the id itself (`system:<uri>`,
 * `custom:<file>`), which is the exact string stored on a reminder.
 */
export const reminderSoundSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  kind: reminderSoundKindSchema,
});

export const reminderSoundCatalogSchema = z.object({
  defaultSoundId: z.string().min(1),
  items: z.array(reminderSoundSchema).min(1),
});

/**
 * The repeats the app offers, as the RFC 5545 rules the core stores.
 *
 * The core refuses anything outside this set rather than approximating it, so
 * the two lists have to stay in step.
 */
export const RECURRENCE_RULES = {
  daily: "FREQ=DAILY",
  weekdays: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
  weekly: "FREQ=WEEKLY",
  monthly: "FREQ=MONTHLY",
  yearly: "FREQ=YEARLY",
} as const;

export type RecurrenceId = keyof typeof RECURRENCE_RULES;

export const RECURRENCE_IDS = Object.keys(RECURRENCE_RULES) as RecurrenceId[];

/** The id for a stored rule, or `null` for a reminder that happens once. */
export function recurrenceIdOf(rule: string | null): RecurrenceId | null {
  return (
    RECURRENCE_IDS.find((id) => RECURRENCE_RULES[id] === rule) ?? null
  );
}

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
  recurrence: z.string().nullable(),
});

export type Reminder = z.infer<typeof reminderSchema>;
export type ReminderSound = z.infer<typeof reminderSoundSchema>;
export type ReminderSoundKind = z.infer<typeof reminderSoundKindSchema>;
export type ReminderSoundCatalog = z.infer<typeof reminderSoundCatalogSchema>;

export interface UpsertReminderRequest {
  /** The reminder being edited, or `null` to add another one to the note. */
  readonly reminderId: ReminderId | null;
  readonly noteId: NoteId;
  readonly title: string;
  readonly body: string;
  readonly scheduledAt: number;
  readonly timezone: string;
  readonly sound: string;
  /** RFC 5545 rule, or `null` for a reminder that happens once. */
  readonly recurrence: string | null;
}

/** Every reminder on a note, soonest first. */
export async function listRemindersForNote(noteIdValue: NoteId): Promise<Reminder[]> {
  return callCommand("reminders_list_for_note", z.array(reminderSchema), {
    noteId: noteIdValue,
  });
}

/** A note on the reminders screen, with everything still due on it. */
export const noteWithRemindersSchema = z.object({
  note: noteSummarySchema,
  reminders: z.array(reminderSchema).min(1),
});

export type NoteWithReminders = z.infer<typeof noteWithRemindersSchema>;

const noteWithRemindersPageSchema = z.object({
  items: z.array(noteWithRemindersSchema),
  total: z.number().int(),
  limit: z.number().int(),
  offset: z.number().int(),
  hasMore: z.boolean(),
});

/**
 * The notes with a reminder still to come, soonest first.
 *
 * One call rather than a list of notes followed by a read per card: the screen
 * shows every note's time, and a hundred round trips for a hundred rows would
 * be felt.
 */
export async function listNotesWithReminders(limit = 100): Promise<Page<NoteWithReminders>> {
  return callCommand("notes_list_with_reminders", noteWithRemindersPageSchema, {
    request: { scope: "all_except_trash", sort: "next_reminder", limit },
  });
}

export async function upsertReminderForNote(
  request: UpsertReminderRequest,
): Promise<Reminder> {
  return callCommand("reminders_upsert_for_note", reminderSchema, { request });
}

export async function deleteReminder(reminderIdValue: ReminderId): Promise<null> {
  return callCommand("reminders_delete", z.null(), {
    reminderId: reminderIdValue,
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

/**
 * Re-renders pending reminders in the zone the device is in now.
 *
 * Nothing tells an app that it has changed country, so this is asked on every
 * start. It answers how many reminders moved, which is zero almost always.
 */
export async function reconcileReminderZone(): Promise<number> {
  return callCommand("reminders_reconcile_zone", z.number().int(), {
    timezone: deviceTimeZone(),
  });
}

/**
 * Arms repeating reminders further ahead.
 *
 * A repeat survives on the firings armed in advance of it, because nothing
 * wakes the core when one goes off. Asked on every start.
 */
export async function topUpReminders(): Promise<number> {
  return callCommand("reminders_top_up", z.number().int());
}

export async function listReminderSounds(): Promise<ReminderSoundCatalog> {
  return callCommand("reminder_sounds_list", reminderSoundCatalogSchema);
}

/**
 * Opens the native file picker for a sound of the user's own.
 *
 * Answers the new catalog entry, or `null` when the user backed out of the
 * picker. The core trims anything longer than ten seconds itself. On desktop
 * there is no picker and the core answers with an `unsupported` error instead.
 */
export async function pickCustomReminderSound(): Promise<ReminderSound | null> {
  return callCommand("reminder_sound_pick_custom", reminderSoundSchema.nullable());
}

export async function deleteCustomReminderSound(soundId: string): Promise<null> {
  return callCommand("reminder_sound_delete_custom", z.null(), { soundId });
}

/** Plays a sound once so it can be judged before it is chosen. */
export async function previewReminderSound(soundId: string): Promise<null> {
  return callCommand("reminder_sound_preview", z.null(), { soundId });
}

export async function stopReminderSoundPreview(): Promise<null> {
  return callCommand("reminder_sound_preview_stop", z.null());
}

/**
 * How long the notification's "later" button moves a reminder by.
 *
 * One number for the whole app rather than one per reminder: "later" is how
 * long this person needs before being asked again.
 */
export const snoozeSettingSchema = z.object({
  minutes: z.number().int().positive(),
  offered: z.array(z.number().int().positive()).min(1),
});

export type SnoozeSetting = z.infer<typeof snoozeSettingSchema>;

export async function loadSnoozeSetting(): Promise<SnoozeSetting> {
  return callCommand("reminder_snooze_get", snoozeSettingSchema);
}

/**
 * Stores it and re-arms everything already waiting.
 *
 * The amount travels inside the alarm the OS holds, so the core has to go round
 * and replace them; that is why this can take a moment on a phone with many
 * reminders.
 */
export async function saveSnoozeSetting(minutes: number): Promise<SnoozeSetting> {
  return callCommand("reminder_snooze_save", snoozeSettingSchema, { minutes });
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
