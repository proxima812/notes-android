import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import { createTaskForNote, deleteTask, listTasksForNote, setTaskCompleted } from "../api";

/**
 * The note's checklist.
 *
 * A ticked box in the rich text is formatting; these are rows, which is what
 * lets the app count them and say how much of a note is still open.
 */
export function NoteChecklist({ noteId }: { readonly noteId: NoteId }): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [draft, setDraft] = useState("");

  const tasks = useQuery({ queryKey: ["tasks", noteId], queryFn: () => listTasksForNote(noteId) });

  const refresh = async (): Promise<void> => {
    await client.invalidateQueries({ queryKey: ["tasks", noteId] });
  };

  const add = useMutation({
    mutationFn: (title: string) => createTaskForNote(noteId, title),
    onSuccess: async () => {
      setDraft("");
      await refresh();
    },
  });
  const toggle = useMutation({
    mutationFn: ({ id, completed }: { id: string; completed: boolean }) =>
      setTaskCompleted(id, completed),
    onSuccess: refresh,
  });
  const remove = useMutation({ mutationFn: deleteTask, onSuccess: refresh });

  const error = add.error ?? toggle.error ?? remove.error;
  const items = tasks.data ?? [];
  const done = items.filter((task) => task.completed).length;

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (draft.trim() === "") {
      return;
    }
    add.mutate(draft);
  };

  return (
    <section
      aria-label={t("checklist.title")}
      className="bg-surface-sunken border-border-subtle flex flex-col gap-3 rounded-2xl border p-4"
    >
      <div className="flex items-center gap-2">
        <h3 className="text-content flex-1 text-base font-semibold">{t("checklist.title")}</h3>
        {items.length > 0 && (
          <span className="text-content-muted text-sm tabular-nums">
            {t("checklist.progress", { done, total: items.length })}
          </span>
        )}
      </div>

      {items.length > 0 && (
        <ul className="space-y-1">
          {items.map((task) => (
            <li key={task.id} className="flex min-h-11 items-center gap-3">
              <input
                type="checkbox"
                checked={task.completed}
                aria-label={task.title}
                onChange={() => {
                  toggle.mutate({ id: task.id, completed: !task.completed });
                }}
                className="size-5 shrink-0 accent-accent"
              />
              <span
                className={`flex-1 text-sm ${
                  task.completed ? "text-content-muted line-through" : "text-content"
                }`}
              >
                {task.title}
              </span>
              <button
                type="button"
                aria-label={t("checklist.remove", { title: task.title })}
                onClick={() => {
                  remove.mutate(task.id);
                }}
                className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
              >
                <Trash2 className="size-4" />
              </button>
            </li>
          ))}
        </ul>
      )}

      <form className="flex items-center gap-2" onSubmit={submit}>
        <input
          type="text"
          value={draft}
          aria-label={t("checklist.add")}
          placeholder={t("checklist.add")}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          className="bg-surface border-border-subtle text-content min-h-11 flex-1 rounded-xl border px-3 outline-none focus:border-accent"
        />
        <button
          type="submit"
          aria-label={t("checklist.add")}
          disabled={draft.trim() === "" || add.isPending}
          className="bg-surface-raised border-border-subtle text-content flex size-11 shrink-0 items-center justify-center rounded-xl border disabled:opacity-40"
        >
          <Plus className="size-4" />
        </button>
      </form>

      {error != null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(error, t)}
        </p>
      )}
    </section>
  );
}
