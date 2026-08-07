import { useQueryClient } from "@tanstack/react-query";
import { Check, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";

import {
  deleteCustomReminderSound,
  pickCustomReminderSound,
  previewReminderSound,
  stopReminderSoundPreview,
  type ReminderSoundCatalog,
} from "../api";

interface SoundPickerProps {
  readonly sounds: ReminderSoundCatalog;
  /** The value stored on the reminder: `"default"`, a preset id, or a prefixed uri. */
  readonly selected: string;
  readonly onSelect: (soundId: string) => void;
  readonly onClose: () => void;
}

/** Shared classes for one selectable row, matching the language sheet. */
function rowClasses(selected: boolean): string {
  return `flex min-h-12 w-full items-center justify-between gap-2 rounded-2xl px-4 text-left ${
    selected ? "bg-accent text-accent-content" : "bg-surface-sunken"
  }`;
}

/**
 * The notification-sound sheet.
 *
 * A sheet rather than a radio list in the form: with device ringtones and the
 * user's own files the list is long, scrolls, and has its own actions (preview,
 * delete, add). Tapping a row selects it and plays it once, so the sound can be
 * judged without leaving the sheet; whatever is playing stops when the sheet
 * goes away.
 */
export function SoundPicker({
  sounds,
  selected,
  onSelect,
  onClose,
}: SoundPickerProps): React.JSX.Element {
  const t = useT();
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useBackGuard(true, onClose);

  // Closing the sheet unmounts it, so this one cleanup covers the scrim, the
  // back gesture and the screen going away.
  useEffect(() => {
    return () => {
      stopReminderSoundPreview().catch(() => undefined);
    };
  }, []);

  const choose = (soundId: string): void => {
    onSelect(soundId);
    // Fire-and-forget: a sound that fails to play should not block choosing it.
    previewReminderSound(soundId).catch(() => undefined);
  };

  const addCustom = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      const added = await pickCustomReminderSound();
      if (added !== null) {
        await client.invalidateQueries({ queryKey: ["reminder-sounds"] });
        choose(added.id);
      }
    } catch (cause: unknown) {
      // On desktop the core answers `unsupported`; the message explains itself.
      setError(describeError(cause, t));
    } finally {
      setBusy(false);
    }
  };

  const removeCustom = async (soundId: string): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      await deleteCustomReminderSound(soundId);
      await client.invalidateQueries({ queryKey: ["reminder-sounds"] });
      if (selected === soundId) {
        onSelect("default");
      }
    } catch (cause: unknown) {
      setError(describeError(cause, t));
    } finally {
      setBusy(false);
    }
  };

  const presets = sounds.items.filter((item) => item.kind === "preset");
  const system = sounds.items.filter((item) => item.kind === "system");
  const custom = sounds.items.filter((item) => item.kind === "custom");
  const defaultLabel =
    sounds.items.find((item) => item.id === sounds.defaultSoundId)?.label ??
    sounds.defaultSoundId;

  const row = (
    id: string,
    label: string,
    extra?: React.ReactNode,
  ): React.JSX.Element => {
    const isSelected = id === selected;
    return (
      <li key={id} className="flex items-center gap-1">
        <button
          type="button"
          role="radio"
          aria-checked={isSelected}
          onClick={() => {
            choose(id);
          }}
          className={rowClasses(isSelected)}
        >
          <span className="min-w-0 flex-1 truncate text-sm font-medium">{label}</span>
          {isSelected && <Check className="size-5 shrink-0" />}
        </button>
        {extra}
      </li>
    );
  };

  const heading = (text: string): React.JSX.Element => (
    <h3 className="text-content-muted mt-3 mb-1 px-1 text-sm">{text}</h3>
  );

  return (
    <>
      <button
        type="button"
        aria-label={t("reminder.soundClose")}
        onClick={onClose}
        className="fixed inset-0 z-40 cursor-default bg-black/50"
      />
      <div
        role="radiogroup"
        aria-label={t("reminder.soundTitle")}
        className="bg-surface-raised border-border-subtle fixed inset-x-0 bottom-0 z-50 mx-auto max-w-md rounded-t-3xl border-t p-4 pb-[calc(1rem+env(safe-area-inset-bottom))] shadow-2xl"
      >
        <div className="bg-border-subtle mx-auto mb-4 h-1 w-10 rounded-full" />
        <h2 className="mb-3 text-lg font-semibold tracking-tight">
          {t("reminder.soundTitle")}
        </h2>

        <div className="max-h-[60vh] overflow-y-auto">
          <ul className="flex flex-col gap-1">
            {row("default", t("reminder.defaultSound", { label: defaultLabel }))}
          </ul>

          {presets.length > 0 && (
            <>
              {heading(t("reminder.soundPresets"))}
              <ul className="flex flex-col gap-1">
                {presets.map((item) => row(item.id, item.label))}
              </ul>
            </>
          )}

          {system.length > 0 && (
            <>
              {heading(t("reminder.soundDevice"))}
              {/* The device list is the long one — dozens of ringtones — so it
                  scrolls inside its own box instead of pushing the app's own
                  sections off the sheet. */}
              <ul className="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
                {system.map((item) => row(item.id, item.label))}
              </ul>
            </>
          )}

          {custom.length > 0 && (
            <>
              {heading(t("reminder.soundCustom"))}
              <ul className="flex flex-col gap-1">
                {custom.map((item) =>
                  row(
                    item.id,
                    item.label,
                    <button
                      type="button"
                      aria-label={t("reminder.soundDelete", { label: item.label })}
                      disabled={busy}
                      onClick={() => {
                        void removeCustom(item.id);
                      }}
                      className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full disabled:opacity-40"
                    >
                      <Trash2 className="size-4" />
                    </button>,
                  ),
                )}
              </ul>
            </>
          )}

          <button
            type="button"
            disabled={busy}
            onClick={() => {
              void addCustom();
            }}
            className="text-content mt-3 flex min-h-12 w-full items-center gap-3 rounded-2xl px-4 text-left disabled:opacity-40"
          >
            <Plus className="size-5 shrink-0" />
            <span className="text-sm font-medium">{t("reminder.soundAdd")}</span>
          </button>
          <p className="text-content-muted px-4 text-xs">{t("reminder.soundAddHint")}</p>

          {error !== null && (
            <p role="alert" className="text-danger mt-2 px-1 text-sm">
              {error}
            </p>
          )}
        </div>
      </div>
    </>
  );
}
