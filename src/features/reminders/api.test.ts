import { describe, expect, it } from "vitest";

import { reminderSchema, reminderSoundCatalogSchema, reminderSoundSchema } from "./api";

const REMINDER_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f";
const NOTE_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e70";
const OCCURRENCE_ID = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e71";

describe("reminder bridge schemas", () => {
  it("accepts the Rust reminder DTO", () => {
    const reminder = reminderSchema.parse({
      id: REMINDER_ID,
      noteId: NOTE_ID,
      occurrenceId: OCCURRENCE_ID,
      title: "Проверить",
      body: "",
      scheduledAt: 1_800_000_000_000,
      timezone: "Asia/Almaty",
      sound: "default",
      effectiveSoundId: "death_and_rebirth",
      effectiveSoundLabel: "Death & Rebirth",
      isExact: true,
      recurrence: null,
    });

    expect(reminder.effectiveSoundId).toBe("death_and_rebirth");
  });

  it("accepts a catalog with sounds of every kind", () => {
    const catalog = reminderSoundCatalogSchema.parse({
      defaultSoundId: "death_and_rebirth",
      items: [
        { id: "death_and_rebirth", label: "Death & Rebirth", kind: "preset" },
        {
          id: "system:content://media/internal/audio/media/42",
          label: "Argon",
          kind: "system",
        },
        { id: "custom:my_song.ogg", label: "my_song", kind: "custom" },
      ],
    });

    expect(catalog.items.map((item) => item.kind)).toEqual([
      "preset",
      "system",
      "custom",
    ]);
  });

  it("rejects an empty sound catalog", () => {
    expect(() =>
      reminderSoundCatalogSchema.parse({
        defaultSoundId: "death_and_rebirth",
        items: [],
      }),
    ).toThrow();
  });

  it("rejects a sound without a kind", () => {
    expect(() =>
      reminderSoundSchema.parse({ id: "death_and_rebirth", label: "Death & Rebirth" }),
    ).toThrow();
  });
});
