import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { noteId } from "@/shared/types/ids";

import { NoteChecklist } from "./NoteChecklist";

const { listTasksForNote, clearTasksForNote } = vi.hoisted(() => ({
  listTasksForNote: vi.fn(),
  clearTasksForNote: vi.fn(),
}));

vi.mock("../api", () => ({
  listTasksForNote,
  clearTasksForNote,
  createTaskForNote: vi.fn(),
  setTaskCompleted: vi.fn(),
  deleteTask: vi.fn(),
}));

const NOTE = noteId("0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e6f");

function renderChecklist(): void {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <NoteChecklist noteId={NOTE} />
    </QueryClientProvider>,
  );
}

describe("NoteChecklist", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listTasksForNote.mockResolvedValue([
      { id: "1", title: "Купить хлеб", completed: false, position: 0 },
      { id: "2", title: "Позвонить", completed: true, position: 1 },
    ]);
    clearTasksForNote.mockResolvedValue(2);
  });

  it("asks before emptying a checklist, and says how much is at stake", async () => {
    renderChecklist();
    await screen.findByText("Купить хлеб");

    await userEvent.click(screen.getByRole("button", { name: "Убрать чек-лист" }));

    expect(screen.getByText("Удалить все пункты (2)?")).toBeInTheDocument();
    expect(clearTasksForNote).not.toHaveBeenCalled();
  });

  it("empties it in one call once the question is answered", async () => {
    renderChecklist();
    await screen.findByText("Купить хлеб");

    await userEvent.click(screen.getByRole("button", { name: "Убрать чек-лист" }));
    await userEvent.click(screen.getByRole("button", { name: "Удалить" }));

    expect(clearTasksForNote).toHaveBeenCalledExactlyOnceWith(NOTE);
  });

  it("offers nothing to remove when there is nothing on the note", async () => {
    listTasksForNote.mockResolvedValue([]);
    renderChecklist();
    await screen.findByRole("button", { name: "Новый пункт" });

    expect(screen.queryByRole("button", { name: "Убрать чек-лист" })).not.toBeInTheDocument();
  });
});
