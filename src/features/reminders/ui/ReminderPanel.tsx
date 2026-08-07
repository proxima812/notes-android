import { ChevronDown, Pencil, Plus, Trash2, X } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { useT } from "@/shared/i18n";

import {
  RECURRENCE_IDS,
  RECURRENCE_RULES,
  recurrenceIdOf,
  type Reminder,
  type ReminderSoundCatalog,
  type RecurrenceId,
} from "../api";
import { SoundPicker } from "./SoundPicker";

export interface ReminderFormValue {
  readonly title: string;
  readonly scheduledAt: number;
  readonly sound: string;
  /** RFC 5545 rule, or `null` when the reminder happens once. */
  readonly recurrence: string | null;
}

interface ReminderPanelProps {
  /** The reminder being edited, or `null` when adding another one. */
  readonly initial: Reminder | null;
  /** Everything already set on this note, soonest first. */
  readonly existing: readonly Reminder[];
  readonly onEdit: (reminder: Reminder | null) => void;
  readonly onRemove: (id: Reminder["id"]) => void;
  readonly sounds: ReminderSoundCatalog;
  readonly noteTitle: string;
  readonly busy: boolean;
  readonly error: string | null;
  /** Preset times as `HH:MM`, already sorted by the core. */
  readonly presets: readonly string[];
  /** Receives the whole set the user now wants, not a change to it. */
  readonly onSavePresets: (next: readonly string[]) => void;
  readonly onSave: (value: ReminderFormValue) => void;
  readonly onClose: () => void;
}

/** Shared classes for the three controls that open a picker. */
const PICKER_CLASSES =
  "picker-field bg-surface border-border-subtle text-content min-h-11 w-full rounded-xl border pl-3 pr-10 outline-none focus:border-accent";

/**
 * Wraps a picker control and draws its arrow.
 *
 * The arrow is ours rather than the browser's because Chromium pins the native
 * one to the very edge of the field — see `.picker-field` in `global.css`. It
 * ignores taps so that hitting it still opens the picker underneath.
 */
function PickerField({
  children,
  className = "",
}: {
  readonly children: React.ReactNode;
  readonly className?: string;
}): React.JSX.Element {
  return (
    <div className={`relative ${className}`}>
      {children}
      <ChevronDown className="text-content-muted pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2" />
    </div>
  );
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

function defaultSoundLabel(sounds: ReminderSoundCatalog): string {
  return (
    sounds.items.find((item) => item.id === sounds.defaultSoundId)?.label ??
    sounds.defaultSoundId
  );
}

export function ReminderPanel({
  initial,
  existing,
  onEdit,
  onRemove,
  sounds,
  noteTitle,
  busy,
  error,
  presets,
  onSavePresets,
  onSave,
  onClose,
}: ReminderPanelProps): React.JSX.Element {
  const t = useT();
  const initialDateTime = initialParts(initial);
  const [title, setTitle] = useState(initial?.title ?? noteTitle);
  const [date, setDate] = useState(initialDateTime.date);
  const [time, setTime] = useState(initialDateTime.time);
  const [sound, setSound] = useState(initial?.sound ?? "default");
  const [repeat, setRepeat] = useState<RecurrenceId | "">(
    recurrenceIdOf(initial?.recurrence ?? null) ?? "",
  );
  const [localError, setLocalError] = useState<string | null>(null);
  const [editingPresets, setEditingPresets] = useState(false);
  const [draftPreset, setDraftPreset] = useState("");
  const [pickingSound, setPickingSound] = useState(false);

  // A system sound can vanish between sessions — the catalog no longer lists
  // its uri — and an honest "unavailable" beats showing the raw id.
  const soundLabel =
    sound === "default"
      ? t("reminder.defaultSound", { label: defaultSoundLabel(sounds) })
      : (sounds.items.find((item) => item.id === sound)?.label ??
        t("reminder.soundMissing"));

  useEffect(() => {
    const next = initialParts(initial);
    setTitle(initial?.title ?? noteTitle);
    setDate(next.date);
    setTime(next.time);
    setSound(initial?.sound ?? "default");
    setRepeat(recurrenceIdOf(initial?.recurrence ?? null) ?? "");
    setLocalError(null);
  }, [initial, noteTitle]);

  const addPreset = (): void => {
    // The core rejects duplicates anyway; catching it here keeps the input from
    // clearing on a tap that changed nothing.
    if (draftPreset === "" || presets.includes(draftPreset)) {
      return;
    }
    onSavePresets([...presets, draftPreset]);
    setDraftPreset("");
  };

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
    onSave({
      title: title.trim(),
      scheduledAt,
      sound,
      recurrence: repeat === "" ? null : RECURRENCE_RULES[repeat],
    });
  };

  return (
    <section
      aria-label={t("reminder.title")}
      data-panel=""
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

      {existing.length > 0 && (
        <ul className="mb-4 space-y-2">
          {existing.map((item) => {
            const editing = item.id === initial?.id;
            return (
              <li
                key={item.id}
                className={`bg-surface flex min-h-11 items-center gap-2 rounded-xl border px-3 ${
                  editing ? "border-accent" : "border-border-subtle"
                }`}
              >
                <button
                  type="button"
                  onClick={() => {
                    onEdit(editing ? null : item);
                  }}
                  className="min-w-0 flex-1 py-2 text-left"
                >
                  <span className="text-content block truncate text-sm">{item.title}</span>
                  <span className="text-content-muted block text-xs">
                    {new Date(item.scheduledAt).toLocaleString()}
                    {item.recurrence !== null && ` · ${t("reminder.repeat")}`}
                  </span>
                </button>
                <button
                  type="button"
                  aria-label={t("reminder.removeOne", { title: item.title })}
                  disabled={busy}
                  onClick={() => {
                    onRemove(item.id);
                  }}
                  className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
                >
                  <Trash2 className="size-4" />
                </button>
              </li>
            );
          })}
        </ul>
      )}

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
            <PickerField className="mt-1">
              <input
                type="date"
                value={date}
                onChange={(event) => {
                  setDate(event.target.value);
                  setLocalError(null);
                }}
                className={PICKER_CLASSES}
              />
            </PickerField>
          </label>
          <label className="text-content-muted block text-sm">
            {t("reminder.time")}
            <PickerField className="mt-1">
              <input
                type="time"
                value={time}
                onChange={(event) => {
                  setTime(event.target.value);
                  setLocalError(null);
                }}
                className={PICKER_CLASSES}
              />
            </PickerField>
          </label>
        </div>

        {/* Preset times sit under the clock they fill in, so the relationship
            between picking "09:00" and the time field changing is visible
            rather than something to work out. */}
        <div className="space-y-2">
          <div className="flex items-end gap-2">
            <label className="text-content-muted block flex-1 text-sm">
              {t("reminder.presets")}
              <PickerField className="mt-1">
                <select
                  value={presets.includes(time) ? time : ""}
                  disabled={presets.length === 0}
                  onChange={(event) => {
                    if (event.target.value === "") {
                      return;
                    }
                    setTime(event.target.value);
                    setLocalError(null);
                  }}
                  className={`${PICKER_CLASSES} disabled:opacity-40`}
                >
                  <option value="">
                    {presets.length === 0
                      ? t("reminder.presetsEmpty")
                      : t("reminder.presetsPlaceholder")}
                  </option>
                  {presets.map((preset) => (
                    <option key={preset} value={preset}>
                      {preset}
                    </option>
                  ))}
                </select>
              </PickerField>
            </label>
            <button
              type="button"
              aria-label={editingPresets ? t("reminder.presetsDone") : t("reminder.presetsEdit")}
              aria-expanded={editingPresets}
              onClick={() => {
                setEditingPresets((open) => !open);
              }}
              className={`flex size-11 shrink-0 items-center justify-center rounded-full ${
                editingPresets ? "text-accent" : "text-content-muted"
              }`}
            >
              <Pencil className="size-5" />
            </button>
          </div>

          {editingPresets && (
            <div className="space-y-2">
              {presets.length > 0 && (
                <ul className="flex flex-wrap gap-2">
                  {presets.map((preset) => (
                    <li key={preset}>
                      <button
                        type="button"
                        aria-label={t("reminder.presetRemove", { time: preset })}
                        onClick={() => {
                          onSavePresets(presets.filter((item) => item !== preset));
                        }}
                        className="bg-surface border-border-subtle text-content flex min-h-11 items-center gap-2 rounded-xl border px-3 text-sm"
                      >
                        {preset}
                        <X className="text-content-muted size-4" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <div className="flex items-center gap-2">
                <div className="flex-1">
                  <PickerField>
                    <input
                      type="time"
                      value={draftPreset}
                      aria-label={t("reminder.presetAdd")}
                      onChange={(event) => {
                        setDraftPreset(event.target.value);
                      }}
                      className={PICKER_CLASSES}
                    />
                  </PickerField>
                </div>
                <button
                  type="button"
                  onClick={addPreset}
                  disabled={draftPreset === ""}
                  className="bg-surface-raised text-content border-border-subtle flex min-h-11 items-center gap-1 rounded-xl border px-3 text-sm font-medium disabled:opacity-40"
                >
                  <Plus className="size-4" />
                  {t("reminder.presetAdd")}
                </button>
              </div>
            </div>
          )}
        </div>

        <label className="text-content-muted block text-sm">
          {t("reminder.repeat")}
          <PickerField className="mt-1">
            <select
              value={repeat}
              onChange={(event) => {
                setRepeat(event.target.value as RecurrenceId | "");
                setLocalError(null);
              }}
              className={PICKER_CLASSES}
            >
              <option value="">{t("reminder.repeatNever")}</option>
              {RECURRENCE_IDS.map((id) => (
                <option key={id} value={id}>
                  {t(`reminder.repeat.${id}` as Parameters<typeof t>[0])}
                </option>
              ))}
            </select>
          </PickerField>
        </label>

        <div className="text-content-muted block text-sm">
          {t("reminder.sound")}
          <PickerField className="mt-1">
            <button
              type="button"
              aria-label={t("reminder.sound")}
              aria-haspopup="dialog"
              aria-expanded={pickingSound}
              onClick={() => {
                setPickingSound(true);
              }}
              className={`${PICKER_CLASSES} flex items-center text-left`}
            >
              <span className="min-w-0 flex-1 truncate">{soundLabel}</span>
            </button>
          </PickerField>
        </div>

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
            {initial === null ? t("reminder.add") : t("reminder.save")}
          </button>
          {initial !== null && (
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                onEdit(null);
              }}
              className="text-content-muted min-h-11 rounded-xl px-4 text-sm font-medium disabled:opacity-40"
            >
              {t("reminder.stopEditing")}
            </button>
          )}
        </div>
      </form>

      {pickingSound && (
        <SoundPicker
          sounds={sounds}
          selected={sound}
          onSelect={(next) => {
            setSound(next);
            setLocalError(null);
          }}
          onClose={() => {
            setPickingSound(false);
          }}
        />
      )}
    </section>
  );
}
