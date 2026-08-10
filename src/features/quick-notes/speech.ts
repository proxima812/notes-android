import { Channel, invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { BridgeError } from "@/shared/api/errors";
import type { LanguageId } from "@/shared/i18n";

/**
 * The dictation plugin, as the screen sees it.
 *
 * This is the one plugin the WebView calls directly. Everything else reaches
 * Android through the Rust core, but how loud the microphone is has to be drawn
 * while it is being heard, and a loudness reading routed through the core would
 * arrive as a stutter. What crosses this boundary is a number, some words, and
 * a finished line — never audio.
 */

const speechEventSchema = z.discriminatedUnion("kind", [
  /** The recogniser is armed and the person may start talking. */
  z.object({ kind: z.literal("ready") }),
  /** A voice was heard. */
  z.object({ kind: z.literal("speaking") }),
  /** The voice stopped; the recogniser is still working. */
  z.object({ kind: z.literal("thinking") }),
  /** Android took the microphone back — the app went to the background. */
  z.object({ kind: z.literal("aborted") }),
  z.object({ kind: z.literal("level"), level: z.number().min(0).max(1) }),
  z.object({ kind: z.literal("partial"), text: z.string() }),
  z.object({ kind: z.literal("final"), text: z.string() }),
  z.object({ kind: z.literal("error"), code: z.string().min(1) }),
]);

export type SpeechEvent = z.infer<typeof speechEventSchema>;

/** Error codes the Kotlin side groups its failures into. */
export const SPEECH_ERROR_CODES = [
  "no_speech",
  "permission",
  "busy",
  "audio",
  "language",
  "offline",
  "unknown",
] as const;

export type SpeechErrorCode = (typeof SPEECH_ERROR_CODES)[number];

/** A code this build knows a sentence for, or `unknown`. */
export function speechErrorCode(code: string): SpeechErrorCode {
  return (SPEECH_ERROR_CODES as readonly string[]).includes(code)
    ? (code as SpeechErrorCode)
    : "unknown";
}

const availabilitySchema = z.object({
  available: z.boolean(),
  granted: z.boolean(),
  offlineGuaranteed: z.boolean(),
});

export type SpeechAvailability = z.infer<typeof availabilitySchema>;

const permissionSchema = z.object({
  granted: z.boolean(),
  /** Refused for good: Android will not show the prompt again. */
  blocked: z.boolean(),
});

export type MicrophoneOutcome = z.infer<typeof permissionSchema>;

const languageSupportSchema = z.object({
  /** False where Android cannot be asked — then the lists mean nothing. */
  known: z.boolean(),
  installed: z.array(z.string()),
  supported: z.array(z.string()),
});

export type SpeechLanguageSupport = z.infer<typeof languageSupportSchema>;

const dictationRequestSchema = z.object({ requested: z.boolean() });

/**
 * The recognition tag each interface language would like.
 *
 * Android wants a full BCP 47 tag and does poorly with a bare `ru`, so each
 * language names a region. Whether the device can actually recognise it is a
 * different question, and [`chooseRecognitionTag`] is where that is settled.
 */
const RECOGNITION_TAGS: Readonly<Record<LanguageId, string>> = {
  ru: "ru-RU",
  en: "en-US",
  es: "es-ES",
  kk: "kk-KZ",
  tt: "tt-RU",
  ba: "ba-RU",
  crh: "crh-UA",
  zh: "zh-CN",
};

export function recognitionTag(language: LanguageId): string {
  return RECOGNITION_TAGS[language];
}

/** `ru-RU` and `ru` are the same language for this purpose. */
function primary(tag: string): string {
  return (tag.toLowerCase().split("-")[0] ?? "").trim();
}

function findTag(wanted: string, available: readonly string[]): string | null {
  const exact = available.find((tag) => tag.toLowerCase() === wanted.toLowerCase());
  if (exact !== undefined) {
    return exact;
  }
  return available.find((tag) => primary(tag) === primary(wanted)) ?? null;
}

export interface ChosenLanguage {
  /** The tag the recogniser will be started with. */
  readonly tag: string;
  /**
   * Why this one. `ui` is the language the app is being read in; `device` is
   * the phone's own; `fallback` is neither, and the screen says so out loud
   * rather than letting someone wonder why their words came back as nonsense.
   */
  readonly reason: "ui" | "device" | "fallback" | "unknown";
}

/**
 * Picks the language to listen in.
 *
 * The app speaks eight languages and Android recognises a different eight:
 * Tatar and Bashkir have no recogniser anywhere, so someone reading the app in
 * Tatar and dictating in Russian — as they do — must not be handed a dead
 * button. The order is the interface language, then whatever the phone itself
 * is set to, then Russian and English as the two this parser understands
 * anyway.
 *
 * A device that cannot be asked (before Android 13) keeps the interface
 * language: guessing on no information would break the majority case to spare
 * the minority one.
 */
export function chooseRecognitionTag(
  language: LanguageId,
  deviceTags: readonly string[],
  support: SpeechLanguageSupport,
): ChosenLanguage {
  const wanted = recognitionTag(language);
  if (!support.known) {
    return { tag: wanted, reason: "unknown" };
  }

  // Installed models first: one that is merely "supported" may need a download
  // the person never asked for.
  const pools = [support.installed, support.supported].filter((pool) => pool.length > 0);
  const preferences: readonly { tag: string; reason: ChosenLanguage["reason"] }[] = [
    { tag: wanted, reason: "ui" },
    ...deviceTags.map((tag) => ({ tag, reason: "device" as const })),
    { tag: "ru-RU", reason: "fallback" as const },
    { tag: "en-US", reason: "fallback" as const },
  ];

  for (const pool of pools) {
    for (const preference of preferences) {
      const found = findTag(preference.tag, pool);
      if (found !== null) {
        return { tag: found, reason: preference.reason };
      }
    }
  }

  // Nothing matched, but the device did answer: take the first thing it can do
  // rather than the tag it has already said no to.
  const anything = pools[0]?.[0];
  return anything === undefined
    ? { tag: wanted, reason: "unknown" }
    : { tag: anything, reason: "fallback" };
}

/** What the phone itself is set to, in preference order. */
export function deviceLanguageTags(): readonly string[] {
  if (typeof navigator === "undefined") {
    return [];
  }
  return navigator.languages ?? [navigator.language];
}

async function call<T extends z.ZodType>(
  command: string,
  schema: T,
  args?: Record<string, unknown>,
): Promise<z.infer<T>> {
  let raw: unknown;
  try {
    raw = await invoke(`plugin:speech|${command}`, args);
  } catch (cause: unknown) {
    throw new BridgeError(command, `Распознавание речи: «${command}» не выполнилось`, {
      cause,
    });
  }

  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    throw new BridgeError(command, `Распознавание речи ответило неожиданно`, {
      cause: parsed.error,
    });
  }
  return parsed.data;
}

/**
 * Whether the device can recognise speech, whether it is allowed to listen, and
 * whether it can promise to do it offline.
 *
 * Asked every time the sheet opens rather than cached: all three can change in
 * the Android settings while the app is in the background.
 */
export async function speechAvailability(): Promise<SpeechAvailability> {
  return call("availability", availabilitySchema);
}

export async function requestMicrophone(): Promise<MicrophoneOutcome> {
  return call("request_permission", permissionSchema);
}

export async function speechLanguageSupport(): Promise<SpeechLanguageSupport> {
  return call("language_support", languageSupportSchema);
}

/** Asks Android to fetch the offline model for a language. */
export async function downloadLanguageModel(language: string): Promise<void> {
  await call("download_language", z.unknown(), { language });
}

/** Opens this app's page in the Android settings. */
export async function openAppSettings(): Promise<void> {
  await call("open_app_settings", z.unknown());
}

/** Whether the launcher shortcut asked for dictation. Answering clears it. */
export async function takeDictationRequest(): Promise<boolean> {
  const outcome = await call("take_dictation_request", dictationRequestSchema);
  return outcome.requested;
}

/**
 * Starts one dictation. Resolves once the recogniser is armed; everything heard
 * arrives on `onEvent`.
 *
 * `preferOffline` is not a parameter. The app has no server and no account, and
 * a dictation that quietly went to one would break that promise in the one
 * place nobody would think to look.
 */
export async function startDictation(
  tag: string,
  onEvent: (event: SpeechEvent) => void,
): Promise<void> {
  const channel = new Channel<unknown>();
  channel.onmessage = (message: unknown): void => {
    const parsed = speechEventSchema.safeParse(message);
    // An event shape this build does not know is dropped rather than thrown
    // from a callback nobody can catch: the dictation carries on, and the
    // worst case is a meter that misses a frame.
    if (parsed.success) {
      onEvent(parsed.data);
    }
  };

  await call("start", z.unknown(), {
    language: tag,
    preferOffline: true,
    onEvent: channel,
  });
}

/** Stops listening and keeps what was heard. */
export async function stopDictation(): Promise<void> {
  await call("stop", z.unknown());
}

/** Stops listening and throws away what was heard. */
export async function cancelDictation(): Promise<void> {
  await call("cancel", z.unknown());
}
