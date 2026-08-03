import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { reminderSchema } from "../api";
import { ReminderPanel, localDateTimeToMillis } from "./ReminderPanel";

const sounds = {
  defaultSoundId: "death_and_rebirth",
  items: [{ id: "death_and_rebirth", label: "Death & Rebirth" }],
} as const;

const presets = ["07:00", "09:00", "12:00", "16:00", "18:00", "23:00"] as const;

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
        presets={presets}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={vi.fn()}
        onSave={onSave}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.clear(screen.getByLabelText("Дата"));
    await user.type(screen.getByLabelText("Дата"), "2030-01-02");
    await user.clear(screen.getByLabelText("Время"));
    await user.type(screen.getByLabelText("Время"), "03:04");
    await user.click(screen.getByRole("button", { name: "Добавить напоминание" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Проверить",
        sound: "default",
        scheduledAt: new Date("2030-01-02T03:04:00").getTime(),
      }),
    );
  });

  it("prefills the reminder being edited and removes one from the list", async () => {
    const onRemove = vi.fn();
    const initial = reminderSchema.parse({
      id: "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f",
      noteId: "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e70",
      occurrenceId: "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e71",
      title: "Выпить воду",
      body: "",
      scheduledAt: new Date("2030-01-02T03:04:00").getTime(),
      timezone: "Asia/Almaty",
      sound: "death_and_rebirth",
      effectiveSoundId: "death_and_rebirth",
      effectiveSoundLabel: "Death & Rebirth",
      isExact: false,
      recurrence: null,
    });

    render(
      <ReminderPanel
        initial={initial}
        sounds={sounds}
        noteTitle="Заметка"
        busy={false}
        error={null}
        presets={presets}
        existing={[initial]}
        onEdit={vi.fn()}
        onRemove={onRemove}
        onSavePresets={vi.fn()}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByDisplayValue("Выпить воду")).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /^Death & Rebirth$/ }),
    ).toBeChecked();
    expect(screen.getByText(/Android может доставить/)).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Удалить напоминание «Выпить воду»" }),
    );
    expect(onRemove).toHaveBeenCalledWith(initial.id);
  });

  it("fills the time from a preset and saves that time", async () => {
    const onSave = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        presets={presets}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={vi.fn()}
        onSave={onSave}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.clear(screen.getByLabelText("Дата"));
    await user.type(screen.getByLabelText("Дата"), "2030-01-02");
    await user.selectOptions(screen.getByLabelText("Готовое время"), "09:00");
    await user.click(screen.getByRole("button", { name: "Добавить напоминание" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        scheduledAt: new Date("2030-01-02T09:00:00").getTime(),
      }),
    );
  });

  it("removes one of the shipped presets and adds one of the user's own", async () => {
    const onSavePresets = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        presets={presets}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={onSavePresets}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Изменить шаблоны времени" }));

    // Deleting a default is the same call as adding a custom one: the whole set
    // is sent, so what matters is that ours is gone from it.
    await user.click(screen.getByRole("button", { name: "Удалить шаблон 07:00" }));
    expect(onSavePresets).toHaveBeenCalledWith(["09:00", "12:00", "16:00", "18:00", "23:00"]);

    await user.type(screen.getByLabelText("Добавить"), "06:45");
    await user.click(screen.getByRole("button", { name: "Добавить" }));
    expect(onSavePresets).toHaveBeenLastCalledWith([...presets, "06:45"]);
  });

  it("saves the chosen repeat as the rule the core stores", async () => {
    const onSave = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        presets={presets}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={vi.fn()}
        onSave={onSave}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.clear(screen.getByLabelText("Дата"));
    await user.type(screen.getByLabelText("Дата"), "2030-01-02");
    await user.selectOptions(screen.getByLabelText("Повтор"), "weekdays");
    await user.click(screen.getByRole("button", { name: "Добавить напоминание" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ recurrence: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR" }),
    );
  });

  it("saves nothing as the rule when the reminder happens once", async () => {
    const onSave = vi.fn();
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        presets={presets}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={vi.fn()}
        onSave={onSave}
        onClose={vi.fn()}
      />,
    );

    const user = userEvent.setup();
    await user.clear(screen.getByLabelText("Дата"));
    await user.type(screen.getByLabelText("Дата"), "2030-01-02");
    await user.click(screen.getByRole("button", { name: "Добавить напоминание" }));

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ recurrence: null }));
  });

  it("offers no preset picker once the user has deleted every one", () => {
    render(
      <ReminderPanel
        initial={null}
        sounds={sounds}
        noteTitle="Проверить"
        busy={false}
        error={null}
        presets={[]}
        existing={[]}
        onEdit={vi.fn()}
        onRemove={vi.fn()}
        onSavePresets={vi.fn()}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Готовое время")).toBeDisabled();
    expect(screen.getByText(/Шаблонов нет/)).toBeInTheDocument();
  });
});
