import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

import {
  clearTasksForNote,
  createTaskForNote,
  deleteTask,
  listTasksForNote,
  setTaskCompleted,
} from "../api";

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
  // Asking is the confirmation; there is no dialog to dismiss by accident.
  const [confirming, setConfirming] = useState(false);

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
  const clear = useMutation({
    mutationFn: () => clearTasksForNote(noteId),
    onSuccess: async () => {
      setConfirming(false);
      await refresh();
    },
  });

  const error = add.error ?? toggle.error ?? remove.error ?? clear.error;
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
      // Marked as a panel so a coloured note can restyle it in CSS: see
      // `.note-surface [data-panel]` in global.css.
      data-panel=""
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
                className="checkbox"
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

      {/* A checklist opened by mistake is taken back in one act rather than a
          row at a time. It asks first, because unlike the tap that made it this
          one cannot be undone by tapping again. */}
      {items.length > 0 &&
        (confirming ? (
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-content flex-1 text-sm">
              {t("checklist.clearConfirm", { count: items.length })}
            </p>
            <button
              type="button"
              disabled={clear.isPending}
              onClick={() => {
                clear.mutate();
              }}
              className="text-danger border-border-subtle min-h-11 rounded-xl border px-3 text-sm font-medium disabled:opacity-40"
            >
              {t("checklist.clearYes")}
            </button>
            <button
              type="button"
              onClick={() => {
                setConfirming(false);
              }}
              className="text-content-muted min-h-11 rounded-xl px-3 text-sm font-medium"
            >
              {t("common.cancel")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => {
              setConfirming(true);
            }}
            className="text-content-muted min-h-11 self-start px-1 text-sm underline-offset-4 hover:underline"
          >
            {t("checklist.clear")}
          </button>
        ))}

      {error != null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(error, t)}
        </p>
      )}
    </section>
  );
}
