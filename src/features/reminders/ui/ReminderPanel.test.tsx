import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReminderPanel, localDateTimeToMillis } from "./ReminderPanel";

const sounds = {
  defaultSoundId: "death_and_rebirth",
  items: [{ id: "death_and_rebirth", label: "Death & Rebirth" }],
} as const;

describe("ReminderPanel", () => {
  it("combines local date and time into milliseconds", () => {
    expect(localDateTimeToMillis("2030-01-02", "03:04")).toBe(
      new Date("2030-01-02T03:04:00").getTime(),
    );
  });

  it("submits the selected preset without closing the form", async () => {
    const onSave = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        onSave={onSave}
        onDelete={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.clear(screen.getByLabelText("Дата"));
    await user.type(screen.getByLabelText("Дата"), "2030-01-02");
    await user.clear(screen.getByLabelText("Время"));
    await user.type(screen.getByLabelText("Время"), "03:04");
    await user.click(screen.getByRole("button", { name: "Сохранить напоминание" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Проверить",
        sound: "default",
        scheduledAt: new Date("2030-01-02T03:04:00").getTime(),
      }),
    );
  });
});
