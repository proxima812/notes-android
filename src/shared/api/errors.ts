import { z } from "zod";

/**
 * Error taxonomy mirrored from `AppError` in the Rust core.
 *
 * The core never sends stack traces, SQL or filesystem paths across the bridge;
 * `message` is already a finished, user-facing Russian sentence and `details`
 * only ever carries small structured facts the UI needs to react (for example
 * which permission is missing).
 */
export const appErrorKindSchema = z.enum([
  "database",
  "validation",
  "reminder",
  "notification",
  "file_system",
  "encryption",
  "backup",
  "import",
  "platform",
  "not_found",
  "conflict",
  "unknown",
]);

export type AppErrorKind = z.infer<typeof appErrorKindSchema>;

export const appErrorSchema = z.object({
  kind: appErrorKindSchema,
  /** Stable machine-readable code, e.g. `exact_alarm_permission_denied`. */
  code: z.string().min(1),
  /** Ready-to-display Russian message. */
  message: z.string().min(1),
  /** Optional structured payload; never contains note content. */
  details: z.record(z.string(), z.string()).optional(),
});

export type AppErrorDto = z.infer<typeof appErrorSchema>;

/** Thrown by `callCommand` when the core reports a failure. */
export class AppError extends Error {
  readonly kind: AppErrorKind;
  readonly code: string;
  readonly details: Readonly<Record<string, string>>;

  constructor(dto: AppErrorDto) {
    super(dto.message);
    this.name = "AppError";
    this.kind = dto.kind;
    this.code = dto.code;
    this.details = dto.details ?? {};
  }

  /** True when the user can fix this by changing an Android setting. */
  get isActionable(): boolean {
    return (
      this.code === "exact_alarm_permission_denied" ||
      this.code === "notification_permission_denied" ||
      this.code === "storage_permission_denied"
    );
  }
}

/**
 * Raised when the bridge itself misbehaves — the command is missing, the
 * WebView is detached, or the payload does not match the agreed contract.
 * Distinct from `AppError`, which is a normal, expected outcome.
 */
export class BridgeError extends Error {
  readonly command: string;

  constructor(command: string, message: string, options?: { cause: unknown }) {
    super(message, options);
    this.name = "BridgeError";
    this.command = command;
  }
}

/** Converts any thrown value into a message safe to show to the user. */
export function describeError(error: unknown): string {
  if (error instanceof AppError) {
    return error.message;
  }
  if (error instanceof BridgeError) {
    return "Ядро приложения не отвечает. Попробуйте перезапустить приложение.";
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Произошла неизвестная ошибка.";
}
