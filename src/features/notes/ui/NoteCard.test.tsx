import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import type { NoteSummary } from "@/features/notes/api";

import { NoteCard } from "./NoteCard";

const note: NoteSummary = {
  id: "note_1" as NoteSummary["id"],
  title: "План на день",
  preview: "Главное за день",
  color: null,
  isArchived: false,
  noteType: "text",
  createdAt: 0,
  updatedAt: 0,
  isPinned: false,
  isFavorite: false,
  wordCount: 3,
  deletedAt: null,
  tags: [],
};

beforeAll(() => {
  // jsdom has no pointer capture; the gesture only needs the calls to succeed.
  Element.prototype.setPointerCapture = vi.fn();
  Element.prototype.releasePointerCapture = vi.fn();
});

function setup() {
  const onOpen = vi.fn();
  const onArchive = vi.fn();
  const onDelete = vi.fn();
  render(
    <ul>
      <NoteCard note={note} busy={false} onOpen={onOpen} onArchive={onArchive} onDelete={onDelete} />
    </ul>,
  );
  const card = screen.getByRole("button", { name: /Открыть заметку/ }).parentElement;
  if (card === null) {
    throw new Error("card element missing");
  }
  return { card, onOpen, onArchive, onDelete };
}

/** One gesture: press, a few moves so the slop is passed, release. */
function swipe(card: HTMLElement, dx: number, dy = 0): void {
  fireEvent.pointerDown(card, { pointerId: 1, clientX: 0, clientY: 0, pointerType: "touch" });
  for (const step of [0.3, 0.7, 1]) {
    fireEvent.pointerMove(card, { pointerId: 1, clientX: dx * step, clientY: dy * step });
  }
  fireEvent.pointerUp(card, { pointerId: 1, clientX: dx, clientY: dy });
}

describe("NoteCard swipe", () => {
  it("archives on a swipe to the left", () => {
    const { card, onArchive, onDelete } = setup();
    swipe(card, -140);
    expect(onArchive).toHaveBeenCalledTimes(1);
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("deletes on a swipe to the right", () => {
    const { card, onArchive, onDelete } = setup();
    swipe(card, 140);
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onArchive).not.toHaveBeenCalled();
  });

  it("does nothing when the card is released short of the commit point", () => {
    const { card, onArchive, onDelete } = setup();
    swipe(card, -60);
    expect(onArchive).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("leaves a mostly vertical drag to the scroller", () => {
    const { card, onArchive, onDelete } = setup();
    swipe(card, -140, -300);
    expect(onArchive).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("does not open the note when the gesture was a swipe", () => {
    const { card, onOpen } = setup();
    swipe(card, -140);
    fireEvent.click(screen.getByRole("button", { name: /Открыть заметку/ }));
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("still opens the note on a plain tap", () => {
    const { onOpen } = setup();
    fireEvent.click(screen.getByRole("button", { name: /Открыть заметку/ }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

describe("NoteCard tags", () => {
  it("shows the tags of the note under its preview", () => {
    render(
      <ul>
        <NoteCard
          note={{ ...note, tags: ["работа", "дом"] }}
          busy={false}
          onOpen={vi.fn()}
          onArchive={vi.fn()}
          onDelete={vi.fn()}
        />
      </ul>,
    );

    expect(screen.getByText("#работа #дом")).toBeInTheDocument();
  });

  it("shows no tag line at all when the note wears none", () => {
    render(
      <ul>
        <NoteCard
          note={note}
          busy={false}
          onOpen={vi.fn()}
          onArchive={vi.fn()}
          onDelete={vi.fn()}
        />
      </ul>,
    );

    expect(screen.queryByText(/#/)).not.toBeInTheDocument();
  });
});
