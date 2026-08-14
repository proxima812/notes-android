import { z } from "zod";

import { noteSchema } from "@/features/notes/api";
import { reminderSchema } from "@/features/reminders/api";
import { callCommand } from "@/shared/api/command";
import { appErrorSchema } from "@/shared/api/errors";
import { deviceTimeZone } from "@/shared/types/ids";

/**
 * The two answers a dictated phrase deliberately does not carry.
 *
 * `leadMinutes` is how far before the time that was said the reminder lands —
 * «встреча, 15:00» with the shipped half hour rings at 14:30. `fallbackTime` is
 * where a phrase with no time in it goes, so «купить молоко» is still a
 * reminder rather than a note nobody opens again.
 */
export const quickNoteSettingsSchema = z.object({
  leadMinutes: z.number().int().min(0),
  fallbackTime: z.string().regex(/^\d{2}:\d{2}$/),
  /**
   * The leads the picker offers, decided by the core the way the reminder time
   * presets are. A stored lead that is not among them is added to the list, so
   * the current setting is always the one that looks chosen.
   */
  offeredLeads: z.array(z.number().int().min(0)).min(1),
  /**
   * Days ahead of today the fallback time lands on: 0 today, 1 tomorrow, and
   * anything larger a day the user asked for outright.
   */
  fallbackDayOffset: z.number().int().min(0),
  /** The furthest day the core accepts, so the field can say no first. */
  maxFallbackDayOffset: z.number().int().positive(),
});

export type QuickNoteSettings = z.infer<typeof quickNoteSettingsSchema>;

/**
 * What one dictation produced.
 *
 * The note is always there; the reminder may not be, and then `reminderError`
 * says why. The core writes the note first on purpose — a permission that is
 * off must not cost someone the words they just said.
 */
export const quickNoteSchema = z.object({
  note: noteSchema,
  reminder: reminderSchema.nullable(),
  reminderError: appErrorSchema.nullable(),
  /**
   * The instant the phrase named, before the lead came off it. The screen needs
   * both: «сказали 15:00, напомню в 14:30» reads as a correct parse, while the
   * alarm time on its own reads as a mishearing.
   */
  spokenAt: z.number().int().nullable(),
  leadMinutes: z.number().int().min(0),
  transcript: z.string(),
});

export type QuickNote = z.infer<typeof quickNoteSchema>;

export async function loadQuickNoteSettings(): Promise<QuickNoteSettings> {
  return callCommand("quick_notes_settings", quickNoteSettingsSchema);
}

export async function saveQuickNoteSettings(
  settings: Pick<QuickNoteSettings, "leadMinutes" | "fallbackTime" | "fallbackDayOffset">,
): Promise<QuickNoteSettings> {
  return callCommand("quick_notes_settings_save", quickNoteSettingsSchema, {
    leadMinutes: settings.leadMinutes,
    fallbackTime: settings.fallbackTime,
    fallbackDayOffset: settings.fallbackDayOffset,
  });
}

/**
 * Hands the recognised sentence to the core, which decides everything else.
 *
 * The zone travels with it because a reminder is a time on a clock: the core
 * has no way to ask Android where the phone is, and «15:00» has to mean 15:00
 * here.
 */
export async function createQuickNote(transcript: string): Promise<QuickNote> {
  return callCommand("quick_notes_create", quickNoteSchema, {
    transcript,
    timezone: deviceTimeZone(),
  });
}
