import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import { deviceTimeZone } from "@/shared/types/ids";

/**
 * What a backup or restore did.
 *
 * `completed: false` is the user backing out of the system picker — an ordinary
 * outcome, not a failure, and the screen says nothing happened rather than
 * showing an error.
 */
export const backupOutcomeSchema = z.object({
  completed: z.boolean(),
  fileName: z.string().nullable(),
  noteCount: z.number().int(),
  reminderCount: z.number().int(),
  sizeBytes: z.number().int(),
});

export const backupRecordSchema = z.object({
  fileName: z.string(),
  sizeBytes: z.number().int(),
  noteCount: z.number().int(),
  createdAt: z.number().int(),
});

export type BackupOutcome = z.infer<typeof backupOutcomeSchema>;
export type BackupRecord = z.infer<typeof backupRecordSchema>;

/**
 * Writes a copy of everything and asks the user where to keep it.
 *
 * The zone travels with the request so the file is named for the day the user
 * was living in, not the one UTC was.
 */
export async function exportBackup(): Promise<BackupOutcome> {
  return callCommand("backup_export", backupOutcomeSchema, {
    timezone: deviceTimeZone(),
  });
}

/** Asks for a backup file and replaces everything in the app with it. */
export async function importBackup(): Promise<BackupOutcome> {
  return callCommand("backup_import", backupOutcomeSchema);
}

export async function latestBackup(): Promise<BackupRecord | null> {
  return callCommand("backup_latest", backupRecordSchema.nullable());
}
