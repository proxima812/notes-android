import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, Upload } from "lucide-react";
import { useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";

import { exportBackup, importBackup, latestBackup, type BackupOutcome } from "../api";

/**
 * Backup and restore.
 *
 * Restoring is behind a confirmation because it is the one action in the app
 * that destroys data without a trash to fish it out of: the current notes are
 * replaced by the file's, and there is no undo for that.
 */
export function BackupSection(): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [confirming, setConfirming] = useState(false);
  const [outcome, setOutcome] = useState<BackupOutcome | null>(null);

  const latest = useQuery({ queryKey: ["backup-latest"], queryFn: latestBackup });

  const invalidateEverything = async (): Promise<void> => {
    // A restore changed every note, every reminder and every setting, so
    // nothing already on screen can be trusted.
    await client.invalidateQueries();
  };

  const save = useMutation({
    mutationFn: exportBackup,
    onSuccess: async (result) => {
      setOutcome(result);
      await client.invalidateQueries({ queryKey: ["backup-latest"] });
    },
  });

  const restore = useMutation({
    mutationFn: importBackup,
    onSuccess: async (result) => {
      setOutcome(result);
      setConfirming(false);
      await invalidateEverything();
    },
    onError: () => {
      setConfirming(false);
    },
  });

  const busy = save.isPending || restore.isPending;
  const error = save.error ?? restore.error;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-content-muted text-sm font-medium">{t("backup.title")}</h2>
      <p className="text-content-muted text-sm">{t("backup.description")}</p>

      <div className="flex flex-col gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            setOutcome(null);
            save.mutate();
          }}
          className="bg-accent text-accent-content flex min-h-11 items-center justify-center gap-2 rounded-xl px-4 text-sm font-medium disabled:opacity-40"
        >
          <Download className="size-4" />
          {save.isPending ? t("backup.exporting") : t("backup.export")}
        </button>

        {confirming ? (
          <div className="border-danger/40 bg-surface-sunken flex flex-col gap-2 rounded-xl border p-3">
            <p className="text-content text-sm font-medium">{t("backup.confirmTitle")}</p>
            <p className="text-content-muted text-sm">{t("backup.confirmBody")}</p>
            <div className="flex gap-2">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setOutcome(null);
                  restore.mutate();
                }}
                className="text-danger border-danger/40 min-h-11 flex-1 rounded-xl border text-sm font-medium disabled:opacity-40"
              >
                {restore.isPending ? t("backup.importing") : t("backup.confirm")}
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setConfirming(false);
                }}
                className="text-content-muted min-h-11 flex-1 rounded-xl text-sm font-medium disabled:opacity-40"
              >
                {t("backup.cancel")}
              </button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setOutcome(null);
              setConfirming(true);
            }}
            className="border-border-subtle text-content flex min-h-11 items-center justify-center gap-2 rounded-xl border px-4 text-sm font-medium disabled:opacity-40"
          >
            <Upload className="size-4" />
            {t("backup.import")}
          </button>
        )}
      </div>

      {error != null && (
        <p role="alert" className="text-danger text-sm">
          {describeError(error, t)}
        </p>
      )}

      {outcome !== null && error == null && (
        <p className="text-content-muted text-sm">
          {outcome.completed
            ? t("backup.done", { count: outcome.noteCount })
            : t("backup.cancelled")}
        </p>
      )}

      {latest.data != null && (
        <p className="text-content-muted/70 text-xs">
          {t("backup.last", {
            date: new Date(latest.data.createdAt).toLocaleString(),
            name: latest.data.fileName,
          })}
        </p>
      )}
    </section>
  );
}
