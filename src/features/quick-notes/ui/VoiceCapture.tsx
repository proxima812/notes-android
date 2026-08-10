import { Download, Mic, MicOff, Settings, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { describeError } from "@/shared/api/errors";
import { useLanguage, useT, type StringKey, type Translate } from "@/shared/i18n";
import { useBackGuard } from "@/shared/lib/useBackGuard";

import { createQuickNote, type QuickNote } from "../api";
import { useLevelMeter } from "../useLevelMeter";
import {
  cancelDictation,
  chooseRecognitionTag,
  deviceLanguageTags,
  downloadLanguageModel,
  openAppSettings,
  requestMicrophone,
  speechAvailability,
  speechErrorCode,
  speechLanguageSupport,
  startDictation,
  stopDictation,
  type ChosenLanguage,
  type SpeechErrorCode,
} from "../speech";

/**
 * Where the sheet is in the one thing it does.
 *
 * `thinking` and `saving` are separate on purpose even though both are a wait:
 * the first is the phone working out what was said and the second is the note
 * being written, and a person who sees the same word for four seconds assumes
 * the app has hung.
 */
type Stage =
  | { readonly kind: "checking" }
  | { readonly kind: "denied"; readonly blocked: boolean }
  | { readonly kind: "unavailable" }
  | { readonly kind: "listening" }
  | { readonly kind: "thinking" }
  | { readonly kind: "saving" }
  | { readonly kind: "failed"; readonly message: string; readonly code: SpeechErrorCode | null };

const ERROR_KEYS: Readonly<Record<SpeechErrorCode, StringKey>> = {
  no_speech: "speech.noSpeech",
  permission: "speech.permission",
  busy: "speech.busy",
  audio: "speech.audio",
  language: "speech.language",
  offline: "speech.offline",
  unknown: "speech.unknown",
};

/**
 * How long each wait is given before it is called a failure.
 *
 * Neither stage can end by itself: both are waiting for an event that Android
 * may never send — the recogniser is destroyed when the app is paused, and
 * `stopListening` is documented to deliver a result but does not always. A
 * frozen sheet with a greyed-out button is the one state with no way out, so
 * both waits have a deadline.
 */
const THINKING_TIMEOUT_MS = 8_000;
const SAVING_TIMEOUT_MS = 6_000;

/** One short buzz when the microphone opens, two when the note is written. */
function buzz(pattern: number | readonly number[]): void {
  try {
    navigator.vibrate?.(pattern as number | number[]);
  } catch {
    // A phone that will not vibrate is not a reason to stop dictating.
  }
}

interface VoiceCaptureProps {
  readonly onCreated: (note: QuickNote) => void;
  readonly onClose: () => void;
}

/**
 * The dictation sheet.
 *
 * One press, one sentence, one note. The circle in the middle grows with the
 * microphone so that the answer to "is it hearing me?" is on the screen rather
 * than in a spinner, and the words appear under it as they are recognised —
 * partial text is the only honest progress bar dictation has.
 *
 * Nothing here decides what the words mean. The finished line goes to the core,
 * which pulls out the time, names the note and places the reminder ahead of it.
 */
export function VoiceCapture({ onCreated, onClose }: VoiceCaptureProps): React.JSX.Element {
  const t = useT();
  const [language] = useLanguage();
  const [stage, setStage] = useState<Stage>({ kind: "checking" });
  const [heard, setHeard] = useState("");
  const [chosen, setChosen] = useState<ChosenLanguage | null>(null);
  const [offlineGuaranteed, setOfflineGuaranteed] = useState(true);

  const { level, push: pushLevel } = useLevelMeter();

  // The sheet can be closed while a dictation is in flight, and a late event
  // must not set state on a component that is gone. Armed in the effect body as
  // well as cleared in the cleanup: React re-runs both in development, and a
  // flag that is only ever cleared stays cleared.
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      cancelDictation().catch(() => undefined);
    };
  }, []);

  // A wait that never ends is the one state with no way out, so each one is
  // given a deadline that turns it into an ordinary failure.
  const watchdog = useRef<number | null>(null);
  const clearWatchdog = useCallback((): void => {
    if (watchdog.current !== null) {
      window.clearTimeout(watchdog.current);
      watchdog.current = null;
    }
  }, []);
  useEffect(() => clearWatchdog, [clearWatchdog]);

  const fail = useCallback(
    (message: string, code: SpeechErrorCode | null = null): void => {
      clearWatchdog();
      if (alive.current) {
        setStage({ kind: "failed", message, code });
      }
    },
    [clearWatchdog],
  );

  const wait = useCallback(
    (milliseconds: number, message: string): void => {
      clearWatchdog();
      watchdog.current = window.setTimeout(() => {
        fail(message);
      }, milliseconds);
    },
    [clearWatchdog, fail],
  );

  const save = useCallback(
    async (transcript: string): Promise<void> => {
      if (transcript.trim() === "") {
        fail(t("speech.noSpeech"), "no_speech");
        return;
      }
      setStage({ kind: "saving" });
      wait(SAVING_TIMEOUT_MS, t("speech.unknown"));
      try {
        const created = await createQuickNote(transcript);
        clearWatchdog();
        buzz([30, 60, 30]);
        // Not gated on this sheet still being open: the note exists either way,
        // and the library — which is still mounted — has to hear about it or it
        // shows a list without the note that was just made.
        onCreated(created);
      } catch (cause: unknown) {
        fail(describeError(cause, t));
      }
    },
    [clearWatchdog, fail, onCreated, t, wait],
  );

  const listen = useCallback(async (): Promise<void> => {
    setHeard("");
    pushLevel(0);
    clearWatchdog();
    setStage({ kind: "checking" });

    try {
      const availability = await speechAvailability();
      if (!alive.current) {
        return;
      }
      setOfflineGuaranteed(availability.offlineGuaranteed);
      if (!availability.available) {
        setStage({ kind: "unavailable" });
        return;
      }
      if (!availability.granted) {
        const outcome = await requestMicrophone();
        if (!alive.current) {
          return;
        }
        if (!outcome.granted) {
          setStage({ kind: "denied", blocked: outcome.blocked });
          return;
        }
      }

      // Which language to listen in is the device's answer, not the app's: the
      // interface speaks eight languages and Android recognises a different
      // eight.
      const support = await speechLanguageSupport();
      const pick = chooseRecognitionTag(language, deviceLanguageTags(), support);
      if (!alive.current) {
        return;
      }
      setChosen(pick);

      await startDictation(pick.tag, (event) => {
        if (!alive.current) {
          return;
        }
        switch (event.kind) {
          case "ready":
            buzz(30);
            clearWatchdog();
            setStage({ kind: "listening" });
            break;
          case "speaking":
            setStage({ kind: "listening" });
            break;
          case "level":
            pushLevel(event.level);
            break;
          case "partial":
            setHeard(event.text);
            break;
          case "thinking":
            pushLevel(0);
            setStage({ kind: "thinking" });
            wait(THINKING_TIMEOUT_MS, t("speech.unknown"));
            break;
          case "final":
            clearWatchdog();
            setHeard(event.text);
            void save(event.text);
            break;
          case "aborted":
            // Android took the microphone back while the app was in the
            // background. Nothing more is coming on this channel.
            fail(t("speech.aborted"));
            break;
          case "error":
            fail(t(ERROR_KEYS[speechErrorCode(event.code)]), speechErrorCode(event.code));
            break;
        }
      });
    } catch (cause: unknown) {
      fail(describeError(cause, t));
    }
  }, [clearWatchdog, fail, language, pushLevel, save, t, wait]);

  // One dictation starts with the sheet: opening it *is* the request. Asking
  // for a second press would make the feature two presses, which is the thing
  // it exists to avoid.
  useEffect(() => {
    void listen();
    // Deliberately once. Re-running on every render would restart the
    // recogniser mid-sentence; the retry button is the way back in.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useBackGuard(true, onClose);

  // A dialog that never takes focus is a dialog a screen reader walks straight
  // past into the library underneath.
  const closeButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const previous = document.activeElement;
    closeButton.current?.focus();
    return () => {
      if (previous instanceof HTMLElement) {
        previous.focus();
      }
    };
  }, []);

  const listening = stage.kind === "listening";
  // The halo is the meter. Scale rather than height because the shape is a
  // circle, and a circle that grows reads as loudness without a legend.
  const halo = 1 + level * 0.8;
  const languageNote =
    chosen !== null && (chosen.reason === "device" || chosen.reason === "fallback")
      ? t("quick.listeningIn", { language: languageName(chosen.tag, language) })
      : null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("quick.title")}
      className="bg-surface/95 fixed inset-0 z-50 flex flex-col items-center justify-center gap-8 p-6 backdrop-blur"
    >
      <button
        ref={closeButton}
        type="button"
        aria-label={t("quick.close")}
        onClick={onClose}
        className="text-content-muted absolute top-4 right-4 flex size-11 items-center justify-center rounded-full"
      >
        <X className="size-5" />
      </button>

      <div className="relative flex size-40 items-center justify-center">
        {listening && (
          <span
            aria-hidden
            style={{ transform: `scale(${halo})`, opacity: 0.15 + level * 0.35 }}
            className="bg-accent absolute inset-0 rounded-full transition-[transform,opacity] duration-75"
          />
        )}
        <span
          className={`relative flex size-24 items-center justify-center rounded-full ${
            listening ? "bg-accent text-accent-content" : "bg-surface-raised text-content-muted"
          }`}
        >
          {stage.kind === "denied" || stage.kind === "unavailable" ? (
            <MicOff className="size-10" />
          ) : (
            <Mic className="size-10" />
          )}
        </span>
      </div>

      {/* Everything that changes while the person waits, announced as it
          changes: with the screen off or TalkBack on, this block is the whole
          interface. */}
      <div
        role="status"
        aria-live="polite"
        className="flex min-h-28 w-full max-w-md flex-col items-center gap-2 text-center"
      >
        <p className="text-content-muted text-sm">{statusLine(stage, t)}</p>

        {/* What the recogniser has so far. It is the only progress this kind of
            waiting can honestly show, and it also lets someone see a misheard
            word before the note is made. */}
        {heard !== "" && <p className="text-content text-xl font-medium">{heard}</p>}

        {stage.kind === "checking" && heard === "" && (
          <p className="text-content-muted text-sm">{t("quick.hint")}</p>
        )}
        {stage.kind === "failed" && <p className="text-danger text-sm">{stage.message}</p>}
        {stage.kind === "denied" && (
          <p className="text-danger text-sm">
            {stage.blocked ? t("quick.deniedForGood") : t("quick.denied")}
          </p>
        )}
        {stage.kind === "unavailable" && (
          <p className="text-danger text-sm">{t("quick.unavailable")}</p>
        )}
        {languageNote !== null && (
          <p className="text-content-muted text-xs">{languageNote}</p>
        )}
        {!offlineGuaranteed && (
          <p className="text-content-muted text-xs">{t("quick.offlineNotGuaranteed")}</p>
        )}
      </div>

      {/* The repair the failure implies, when there is one. */}
      {stage.kind === "failed" && stage.code === "language" && chosen !== null && (
        <button
          type="button"
          onClick={() => {
            downloadLanguageModel(chosen.tag).catch(() => undefined);
          }}
          className="border-border-subtle text-content flex min-h-11 items-center gap-2 rounded-2xl border px-4 text-sm"
        >
          <Download className="size-4" />
          {t("quick.downloadModel")}
        </button>
      )}

      <div className="flex w-full max-w-md gap-2">
        <button
          type="button"
          onClick={onClose}
          className="border-border-subtle text-content-muted min-h-12 flex-1 rounded-2xl border"
        >
          {t("quick.cancel")}
        </button>

        {listening ? (
          // Stopping keeps what was heard: this is "done", not "cancel". Some
          // recognisers wait a long time for a silence that the person has
          // already decided is the end of the sentence.
          <button
            type="button"
            onClick={() => {
              setStage({ kind: "thinking" });
              wait(THINKING_TIMEOUT_MS, t("speech.unknown"));
              stopDictation().catch((cause: unknown) => {
                fail(describeError(cause, t));
              });
            }}
            className="bg-accent text-accent-content min-h-12 flex-1 rounded-2xl font-medium"
          >
            {t("quick.done")}
          </button>
        ) : stage.kind === "denied" && stage.blocked ? (
          // Android will not show the prompt again, so a button that asks for it
          // would do nothing at all. This one goes where the switch actually is.
          <button
            type="button"
            onClick={() => {
              openAppSettings().catch(() => undefined);
            }}
            className="bg-accent text-accent-content flex min-h-12 flex-1 items-center justify-center gap-2 rounded-2xl font-medium"
          >
            <Settings className="size-4" />
            {t("quick.openSettings")}
          </button>
        ) : (
          <button
            type="button"
            disabled={stage.kind === "saving" || stage.kind === "unavailable"}
            onClick={() => {
              void listen();
            }}
            className="bg-accent text-accent-content min-h-12 flex-1 rounded-2xl font-medium disabled:opacity-40"
          >
            {stage.kind === "denied" ? t("quick.allow") : t("quick.again")}
          </button>
        )}
      </div>
    </div>
  );
}

function statusLine(stage: Stage, t: Translate): string {
  switch (stage.kind) {
    case "checking":
      return t("quick.starting");
    case "listening":
      return t("quick.listening");
    case "thinking":
      return t("quick.thinking");
    case "saving":
      return t("quick.saving");
    default:
      return t("quick.title");
  }
}

/**
 * The name of the language being listened in, written in the language the app
 * is being read in — "русский" for someone reading the Tatar interface.
 */
function languageName(tag: string, reading: string): string {
  try {
    return new Intl.DisplayNames([reading], { type: "language" }).of(tag) ?? tag;
  } catch {
    return tag;
  }
}
