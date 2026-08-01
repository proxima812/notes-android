import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { AppError, BridgeError, appErrorSchema } from "./errors";

/**
 * Every Rust command answers with this envelope, so a failure is an ordinary
 * value rather than a thrown string with a Rust `Debug` representation in it.
 */
export type CommandResult<T> =
  | { readonly success: true; readonly data: T }
  | { readonly success: false; readonly error: z.infer<typeof appErrorSchema> };

function envelopeSchema<T extends z.ZodType>(
  data: T,
): z.ZodType<CommandResult<z.infer<T>>> {
  return z.discriminatedUnion("success", [
    z.object({ success: z.literal(true), data }),
    z.object({ success: z.literal(false), error: appErrorSchema }),
  ]) as z.ZodType<CommandResult<z.infer<T>>>;
}

/**
 * Calls a Rust command and validates both the envelope and the payload.
 *
 * Nothing crossing the bridge is trusted: a shape mismatch means the TypeScript
 * contract and the Rust DTO have drifted apart, which is a bug worth surfacing
 * loudly rather than letting `undefined` leak into a screen.
 *
 * @throws {AppError} when the core reports a domain failure.
 * @throws {BridgeError} when the call or the response shape is broken.
 */
export async function callCommand<TResponse extends z.ZodType>(
  command: string,
  responseSchema: TResponse,
  args?: Record<string, unknown>,
): Promise<z.infer<TResponse>> {
  let raw: unknown;
  try {
    raw = await invoke(command, args);
  } catch (cause: unknown) {
    throw new BridgeError(command, `Команда «${command}» не выполнилась`, { cause });
  }

  const parsed = envelopeSchema(responseSchema).safeParse(raw);
  if (!parsed.success) {
    throw new BridgeError(
      command,
      `Команда «${command}» вернула неожиданный формат ответа`,
      { cause: parsed.error },
    );
  }

  if (!parsed.data.success) {
    throw new AppError(parsed.data.error);
  }

  return parsed.data.data;
}

/** Commands that report success without a payload. */
export const emptyResponseSchema = z.null();
