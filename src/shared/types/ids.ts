/**
 * Branded identifiers.
 *
 * Every domain identifier is a distinct type, so a `TaskId` can never be passed
 * where a `NoteId` is expected even though both are strings at runtime. The
 * brand is phantom: it exists only in the type system and costs nothing.
 *
 * Values are produced either by Rust (which owns id generation) or by the
 * `*Id()` constructors below, which validate before branding. Never cast.
 */

declare const brand: unique symbol;

type Brand<T, TBrand extends string> = T & { readonly [brand]: TBrand };

export type NoteId = Brand<string, "NoteId">;
export type TaskId = Brand<string, "TaskId">;
export type ReminderId = Brand<string, "ReminderId">;
export type ReminderOccurrenceId = Brand<string, "ReminderOccurrenceId">;
export type AttachmentId = Brand<string, "AttachmentId">;
export type FolderId = Brand<string, "FolderId">;
export type TagId = Brand<string, "TagId">;
export type NoteBlockId = Brand<string, "NoteBlockId">;
export type TemplateId = Brand<string, "TemplateId">;
export type SavedSearchId = Brand<string, "SavedSearchId">;
export type BackupId = Brand<string, "BackupId">;

/** ISO-8601 instant with an explicit offset, e.g. `2026-08-01T09:00:00+03:00`. */
export type IsoDateTime = Brand<string, "IsoDateTime">;

/** IANA time zone name, e.g. `Europe/Moscow`. */
export type TimeZone = Brand<string, "TimeZone">;

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Matches an ISO-8601 instant with a date, a time and an explicit offset.
 * A bare local time is rejected: reminders must never be ambiguous.
 */
const ISO_DATE_TIME_PATTERN =
  /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.\d{1,9})?)?(?:Z|([+-])(\d{2}):(\d{2}))$/;

function daysInMonth(year: number, month: number): number {
  if (month === 2) {
    const isLeap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
    return isLeap ? 29 : 28;
  }
  return month === 4 || month === 6 || month === 9 || month === 11 ? 30 : 31;
}

/** `Area/City`, optionally three-part like `America/Argentina/Salta`, or `UTC`. */
const TIME_ZONE_PATTERN = /^(UTC|[A-Za-z]+(?:_[A-Za-z]+)*(?:\/[A-Za-z0-9+_-]+){1,2})$/;

export class InvalidIdentifierError extends Error {
  constructor(kind: string, value: string) {
    super(`Некорректный идентификатор ${kind}: ${JSON.stringify(value)}`);
    this.name = "InvalidIdentifierError";
  }
}

function uuidBrand<T extends string>(kind: string): (value: string) => Brand<string, T> {
  return (value: string): Brand<string, T> => {
    if (!UUID_PATTERN.test(value)) {
      throw new InvalidIdentifierError(kind, value);
    }
    return value as Brand<string, T>;
  };
}

export const noteId = uuidBrand<"NoteId">("заметки");
export const taskId = uuidBrand<"TaskId">("задачи");
export const reminderId = uuidBrand<"ReminderId">("напоминания");
export const reminderOccurrenceId = uuidBrand<"ReminderOccurrenceId">("срабатывания");
export const attachmentId = uuidBrand<"AttachmentId">("вложения");
export const folderId = uuidBrand<"FolderId">("папки");
export const tagId = uuidBrand<"TagId">("тега");
export const noteBlockId = uuidBrand<"NoteBlockId">("блока");
export const templateId = uuidBrand<"TemplateId">("шаблона");
export const savedSearchId = uuidBrand<"SavedSearchId">("сохранённого поиска");
export const backupId = uuidBrand<"BackupId">("резервной копии");

export function isoDateTime(value: string): IsoDateTime {
  const match = ISO_DATE_TIME_PATTERN.exec(value);
  if (match === null) {
    throw new InvalidIdentifierError("даты и времени", value);
  }

  // `Date.parse` silently rolls impossible dates over — it turns 2026-02-31
  // into 2026-03-03 rather than failing — so the components are checked here.
  const [, rawYear, rawMonth, rawDay, rawHour, rawMinute, rawSecond] = match;
  const year = Number(rawYear);
  const month = Number(rawMonth);
  const day = Number(rawDay);
  const hour = Number(rawHour);
  const minute = Number(rawMinute);
  const second = rawSecond === undefined ? 0 : Number(rawSecond);

  const valid =
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= daysInMonth(year, month) &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59;

  if (!valid) {
    throw new InvalidIdentifierError("даты и времени", value);
  }

  const offsetHours = match[8];
  const offsetMinutes = match[9];
  if (
    offsetHours !== undefined &&
    offsetMinutes !== undefined &&
    (Number(offsetHours) > 14 || Number(offsetMinutes) > 59)
  ) {
    throw new InvalidIdentifierError("даты и времени", value);
  }

  return value as IsoDateTime;
}

export function timeZone(value: string): TimeZone {
  if (!TIME_ZONE_PATTERN.test(value)) {
    throw new InvalidIdentifierError("часового пояса", value);
  }
  return value as TimeZone;
}

/** The zone the device is currently in, used as the default for new reminders. */
export function deviceTimeZone(): TimeZone {
  return timeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
}
