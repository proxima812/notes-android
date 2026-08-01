import { describe, expect, it } from "vitest";

import {
  InvalidIdentifierError,
  deviceTimeZone,
  isoDateTime,
  noteId,
  timeZone,
} from "./ids";

describe("uuid identifiers", () => {
  it("accepts a well-formed uuid", () => {
    const value = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f";
    expect(noteId(value)).toBe(value);
  });

  it("rejects an empty string", () => {
    expect(() => noteId("")).toThrow(InvalidIdentifierError);
  });

  it("rejects a plain word", () => {
    expect(() => noteId("note-1")).toThrow(InvalidIdentifierError);
  });

  it("rejects a uuid with a missing group", () => {
    expect(() => noteId("0193b3b2-4d3c-7c9a-8f2e")).toThrow(InvalidIdentifierError);
  });
});

describe("isoDateTime", () => {
  it("accepts an instant with a positive offset", () => {
    expect(isoDateTime("2026-08-01T09:00:00+03:00")).toBe("2026-08-01T09:00:00+03:00");
  });

  it("accepts a UTC instant", () => {
    expect(isoDateTime("2026-08-01T06:00:00Z")).toBe("2026-08-01T06:00:00Z");
  });

  it("accepts fractional seconds", () => {
    expect(isoDateTime("2026-08-01T06:00:00.123Z")).toBe("2026-08-01T06:00:00.123Z");
  });

  it("rejects a local time without an offset, which would be ambiguous", () => {
    expect(() => isoDateTime("2026-08-01T09:00:00")).toThrow(InvalidIdentifierError);
  });

  it("rejects a date without a time", () => {
    expect(() => isoDateTime("2026-08-01")).toThrow(InvalidIdentifierError);
  });

  it("rejects a calendar-impossible date", () => {
    expect(() => isoDateTime("2026-02-31T09:00:00Z")).toThrow(InvalidIdentifierError);
  });
});

describe("timeZone", () => {
  it("accepts a two-part zone", () => {
    expect(timeZone("Europe/Moscow")).toBe("Europe/Moscow");
  });

  it("accepts a three-part zone", () => {
    expect(timeZone("America/Argentina/Salta")).toBe("America/Argentina/Salta");
  });

  it("accepts UTC", () => {
    expect(timeZone("UTC")).toBe("UTC");
  });

  it("rejects an offset string", () => {
    expect(() => timeZone("+03:00")).toThrow(InvalidIdentifierError);
  });

  it("rejects an empty string", () => {
    expect(() => timeZone("")).toThrow(InvalidIdentifierError);
  });
});

describe("deviceTimeZone", () => {
  it("returns a zone the validator accepts", () => {
    expect(() => timeZone(deviceTimeZone())).not.toThrow();
  });
});

describe("isoDateTime calendar edges", () => {
  it("accepts 29 February in a leap year", () => {
    expect(isoDateTime("2024-02-29T00:00:00Z")).toBe("2024-02-29T00:00:00Z");
  });

  it("rejects 29 February in a common year", () => {
    expect(() => isoDateTime("2026-02-29T00:00:00Z")).toThrow(InvalidIdentifierError);
  });

  it("rejects 31 April", () => {
    expect(() => isoDateTime("2026-04-31T00:00:00Z")).toThrow(InvalidIdentifierError);
  });

  it("rejects hour 24", () => {
    expect(() => isoDateTime("2026-04-30T24:00:00Z")).toThrow(InvalidIdentifierError);
  });

  it("rejects minute 60", () => {
    expect(() => isoDateTime("2026-04-30T10:60:00Z")).toThrow(InvalidIdentifierError);
  });

  it("rejects an out-of-range offset", () => {
    expect(() => isoDateTime("2026-04-30T10:00:00+20:00")).toThrow(InvalidIdentifierError);
  });
})
