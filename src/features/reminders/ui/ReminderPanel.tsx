import { X } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { useT } from "@/shared/i18n";

import type { Reminder } from "../api";

interface SoundCatalog {
  readonly defaultSoundId: string;
  readonly items: readonly {
    readonly id: string;
    readonly label: string;
  }[];
}

export interface ReminderFormValue {
  readonly title: string;
  readonly scheduledAt: number;
  readonly sound: string;
}

interface ReminderPanelProps {
  readonly initial: Reminder | null;
  readonly sounds: SoundCatalog;
  readonly noteTitle: string;
  readonly busy: boolean;
  readonly error: string | null;
  readonly onSave: (value: ReminderFormValue) => void;
  readonly onDelete: () => void;
  readonly onClose: () => void;
}

export function localDateTimeToMillis(date: string, time: string): number {
  return new Date(`${date}T${time}:00`).getTime();
}

function initialParts(initial: Reminder | null): { date: string; time: string } {
  const value = initial === null ? new Date(Date.now() + 60 * 60 * 1000) : new Date(initial.scheduledAt);
  const local = new Date(value.getTime() - value.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
  return { date: local.slice(0, 10), time: local.slice(11, 16) };
}

function defaultSoundLabel(sounds: SoundCatalog): string {
  return (
    sounds.items.find((item) => item.id === sounds.defaultSoundId)?.label ??
    sounds.defaultSoundId
  );
}

export function ReminderPanel({
  initial,
  sounds,
  noteTitle,
  busy,
  error,
  onSave,
  onDelete,
  onClose,
}: ReminderPanelProps): React.JSX.Element {
  const t = useT();
  const initialDateTime = initialParts(initial);
  const [title, setTitle] = useState(initial?.title ?? noteTitle);
  const [date, setDate] = useState(initialDateTime.date);
  const [time, setTime] = useState(initialDateTime.time);
  const [sound, setSound] = useState(initial?.sound ?? "default");
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    const next = initialParts(initial);
    setTitle(initial?.title ?? noteTitle);
    setDate(next.date);
    setTime(next.time);
    setSound(initial?.sound ?? "default");
    setLocalError(null);
  }, [initial, noteTitle]);

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const scheduledAt = localDateTimeToMillis(date, time);
    if (!Number.isFinite(scheduledAt)) {
      setLocalError(t("reminder.errorWhen"));
      return;
    }
    if (scheduledAt <= Date.now()) {
      setLocalError(t("reminder.errorPast"));
      return;
    }
    if (title.trim().length === 0) {
      setLocalError(t("reminder.errorTitle"));
      return;
    }

    setLocalError(null);
    onSave({ title: title.trim(), scheduledAt, sound });
  };

  return (
    <section
      aria-label={t("reminder.title")}
      className="bg-surface-sunken border-border-subtle rounded-2xl border p-4"
    >
      <div className="mb-3 flex min-h-11 items-center gap-3">
        <h2 className="text-content flex-1 text-base font-semibold">{t("reminder.title")}</h2>
        <button
          type="button"
          aria-label={t("reminder.close")}
          onClick={onClose}
          className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <X className="size-5" />
        </button>
      </div>

      <form className="space-y-4" onSubmit={submit}>
        <label className="text-content-muted block text-sm">
          {t("reminder.name")}
          <input
            type="text"
            value={title}
            onChange={(event) => {
              setTitle(event.target.value);
              setLocalError(null);
            }}
            className="bg-surface border-border-subtle text-content mt-1 min-h-11 w-full rounded-xl border px-3 outline-none focus:border-accent"
          />
        </label>

        <div className="grid grid-cols-2 gap-3">
          <label className="text-content-muted block text-sm">
            {t("reminder.date")}
            <input
              type="date"
              value={date}
              onChange={(event) => {
                setDate(event.target.value);
                setLocalError(null);
              }}
              className="bg-surface border-border-subtle text-content mt-1 min-h-11 w-full rounded-xl border px-3 outline-none focus:border-accent"
            />
          </label>
          <label className="text-content-muted block text-sm">
            {t("reminder.time")}
            <input
              type="time"
              value={time}
              onChange={(event) => {
                setTime(event.target.value);
                setLocalError(null);
              }}
              className="bg-surface border-border-subtle text-content mt-1 min-h-11 w-full rounded-xl border px-3 outline-none focus:border-accent"
            />
          </label>
        </div>

        <fieldset>
          <legend className="text-content-muted mb-1 text-sm">{t("reminder.sound")}</legend>
          <div className="space-y-1">
            <label className="text-content flex min-h-11 items-center gap-3 rounded-xl px-2">
              <input
                type="radio"
                name="reminder-sound"
                value="default"
                checked={sound === "default"}
                onChange={(event) => {
                  setSound(event.target.value);
                }}
                className="size-5 accent-accent"
              />
              <span className="text-sm">{t("reminder.defaultSound", { label: defaultSoundLabel(sounds) })}</span>
            </label>
            {sounds.items.map((item) => (
              <label
                key={item.id}
                className="text-content flex min-h-11 items-center gap-3 rounded-xl px-2"
              >
                <input
                  type="radio"
                  name="reminder-sound"
                  value={item.id}
                  checked={sound === item.id}
                  onChange={(event) => {
                    setSound(event.target.value);
                  }}
                  className="size-5 accent-accent"
                />
                <span className="text-sm">{item.label}</span>
              </label>
            ))}
          </div>
        </fieldset>

        {initial?.isExact === false && (
          <p className="text-content-muted text-sm">
            {t("reminder.delay")}
          </p>
        )}

        {(localError ?? error) !== null && (
          <p role="alert" className="text-danger text-sm">
            {localError ?? error}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="submit"
            disabled={busy}
            className="bg-accent text-accent-content min-h-11 flex-1 rounded-xl px-4 text-sm font-medium disabled:opacity-40"
          >
            {t("reminder.save")}
          </button>
          {initial !== null && (
            <button
              type="button"
              disabled={busy}
              onClick={onDelete}
              className="text-danger min-h-11 rounded-xl px-4 text-sm font-medium disabled:opacity-40"
            >
              {t("reminder.delete")}
            </button>
          )}
        </div>
      </form>
    </section>
  );
}
