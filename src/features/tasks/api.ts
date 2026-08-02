import { z } from "zod";

import { callCommand } from "@/shared/api/command";
import type { NoteId } from "@/shared/types/ids";

export const taskSchema = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  completed: z.boolean(),
  /** The order the user arranged by hand. */
  position: z.number().int(),
});

export const taskProgressSchema = z.object({
  total: z.number().int(),
  completed: z.number().int(),
});

export type Task = z.infer<typeof taskSchema>;
export type TaskProgress = z.infer<typeof taskProgressSchema>;

export async function listTasksForNote(noteId: NoteId): Promise<Task[]> {
  return callCommand("tasks_list_for_note", z.array(taskSchema), { noteId });
}

export async function createTaskForNote(noteId: NoteId, title: string): Promise<Task> {
  return callCommand("tasks_create_for_note", taskSchema, { noteId, title });
}

export async function setTaskCompleted(id: string, completed: boolean): Promise<Task> {
  return callCommand("tasks_set_completed", taskSchema, { id, completed });
}

export async function deleteTask(id: string): Promise<null> {
  return callCommand("tasks_delete", z.null(), { id });
}

export async function taskProgressForNote(noteId: NoteId): Promise<TaskProgress> {
  return callCommand("tasks_progress_for_note", taskProgressSchema, { noteId });
}
