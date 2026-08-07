import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  deleteCustomReminderSound,
  pickCustomReminderSound,
  previewReminderSound,
  stopReminderSoundPreview,
  type ReminderSoundCatalog,
} from "../api";
import { SoundPicker } from "./SoundPicker";

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return {
    ...actual,
    pickCustomReminderSound: vi.fn(),
    deleteCustomReminderSound: vi.fn(),
    previewReminderSound: vi.fn(),
    stopReminderSoundPreview: vi.fn(),
  };
});

const catalog: ReminderSoundCatalog = {
  defaultSoundId: "death_and_rebirth",
  items: [
    { id: "death_and_rebirth", label: "Death & Rebirth", kind: "preset" },
    { id: "system:content://media/internal/audio/media/42", label: "Argon", kind: "system" },
    { id: "custom:my_song.ogg", label: "my_song", kind: "custom" },
  ],
};

function renderPicker({
  selected = "default",
  onSelect = vi.fn(),
  onClose = vi.fn(),
}: {
  selected?: string;
  onSelect?: (soundId: string) => void;
  onClose?: () => void;
} = {}): ReturnType<typeof render> {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <SoundPicker
        sounds={catalog}
        selected={selected}
        onSelect={onSelect}
        onClose={onClose}
      />
    </QueryClientProvider>,
  );
}

describe("SoundPicker", () => {
  beforeEach(() => {
    vi.mocked(previewReminderSound).mockResolvedValue(null);
    vi.mocked(stopReminderSoundPreview).mockResolvedValue(null);
    vi.mocked(deleteCustomReminderSound).mockResolvedValue(null);
    vi.mocked(pickCustomReminderSound).mockResolvedValue(null);
  });

  it("selects a tapped row and previews it without closing", async () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    renderPicker({ onSelect, onClose });

    await userEvent.click(screen.getByRole("radio", { name: "Argon" }));

    expect(onSelect).toHaveBeenCalledWith("system:content://media/internal/audio/media/42");
    expect(previewReminderSound).toHaveBeenCalledWith(
      "system:content://media/internal/audio/media/42",
    );
    expect(onClose).not.toHaveBeenCalled();
  });

  it("marks the selected sound", () => {
    renderPicker({ selected: "custom:my_song.ogg" });

    expect(screen.getByRole("radio", { name: "my_song" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Argon" })).not.toBeChecked();
  });

  it("stops the preview when the sheet goes away", () => {
    const { unmount } = renderPicker();
    unmount();

    expect(stopReminderSoundPreview).toHaveBeenCalled();
  });

  it("deletes a custom sound and falls back to the default when it was chosen", async () => {
    const onSelect = vi.fn();
    renderPicker({ selected: "custom:my_song.ogg", onSelect });

    await userEvent.click(
      screen.getByRole("button", { name: "Удалить звук «my_song»" }),
    );

    expect(deleteCustomReminderSound).toHaveBeenCalledWith("custom:my_song.ogg");
    await waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith("default");
    });
  });

  it("adds a sound from a file and selects it", async () => {
    vi.mocked(pickCustomReminderSound).mockResolvedValue({
      id: "custom:new_song.ogg",
      label: "new_song",
      kind: "custom",
    });
    const onSelect = vi.fn();
    renderPicker({ onSelect });

    await userEvent.click(screen.getByRole("button", { name: /Добавить из файла/ }));

    await waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith("custom:new_song.ogg");
    });
    expect(previewReminderSound).toHaveBeenCalledWith("custom:new_song.ogg");
  });

  it("selects nothing when the native picker is cancelled", async () => {
    const onSelect = vi.fn();
    renderPicker({ onSelect });

    await userEvent.click(screen.getByRole("button", { name: /Добавить из файла/ }));

    await waitFor(() => {
      expect(pickCustomReminderSound).toHaveBeenCalled();
    });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("shows the core's message when adding is unsupported", async () => {
    vi.mocked(pickCustomReminderSound).mockRejectedValue(
      new Error("Выбор файла недоступен на этой платформе"),
    );
    renderPicker();

    await userEvent.click(screen.getByRole("button", { name: /Добавить из файла/ }));

    expect(
      await screen.findByText("Выбор файла недоступен на этой платформе"),
    ).toBeInTheDocument();
  });
});
