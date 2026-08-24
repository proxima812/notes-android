import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import type { NoteSummary } from "@/features/notes/api";

import { NoteCard, type NoteCardActions } from "./NoteCard";

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

const inLibrary: NoteCardActions = {
  kind: "library",
  onArchive: vi.fn(),
  onDelete: vi.fn(),
};

beforeAll(() => {
  // jsdom has no pointer capture; the gesture only needs the calls to succeed.
  Element.prototype.setPointerCapture = vi.fn();
  Element.prototype.releasePointerCapture = vi.fn();
});

/** The element the gesture is on: the one the open button sits inside. */
function cardOf(): HTMLElement {
  const card = screen.getByRole("button", { name: /Открыть заметку/ }).parentElement;
  if (card === null) {
    throw new Error("card element missing");
  }
  return card;
}

function setup() {
  const onOpen = vi.fn();
  const onArchive = vi.fn();
  const onDelete = vi.fn();
  render(
    <ul>
      <NoteCard
        note={note}
        busy={false}
        onOpen={onOpen}
        actions={{ kind: "library", onArchive, onDelete }}
      />
    </ul>,
  );
  return { card: cardOf(), onOpen, onArchive, onDelete };
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
          actions={inLibrary}
        />
      </ul>,
    );

    expect(screen.getByText("#работа #дом")).toBeInTheDocument();
  });

  it("shows no tag line at all when the note wears none", () => {
    render(
      <ul>
        <NoteCard note={note} busy={false} onOpen={vi.fn()} actions={inLibrary} />
      </ul>,
    );

    expect(screen.queryByText(/#/)).not.toBeInTheDocument();
  });
});

describe("NoteCard reminder", () => {
  it("says when the soonest reminder goes off", () => {
    const at = new Date();
    at.setDate(at.getDate() + 1);
    at.setHours(8, 0, 0, 0);

    render(
      <ul>
        <NoteCard
          note={note}
          busy={false}
          reminderAt={at.getTime()}
          onOpen={vi.fn()}
          actions={inLibrary}
        />
      </ul>,
    );

    expect(screen.getByText("Завтра, 08:00")).toBeInTheDocument();
  });

  it("says nothing at all on a note that is not going to ask for anything", () => {
    render(
      <ul>
        <NoteCard note={note} busy={false} onOpen={vi.fn()} actions={inLibrary} />
      </ul>,
    );

    expect(screen.queryByLabelText("Напоминание")).not.toBeInTheDocument();
  });
});

describe("NoteCard in the trash", () => {
  const trashed = { ...note, deletedAt: Date.now() };

  it("counts down what is left of the hour, and offers both ways out", () => {
    render(
      <ul>
        <NoteCard
          note={trashed}
          busy={false}
          onOpen={vi.fn()}
          actions={{ kind: "trash", minutesLeft: 47, onRestore: vi.fn(), onPurge: vi.fn() }}
        />
      </ul>,
    );

    expect(screen.getByText("Осталось 47 мин")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Восстановить" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Удалить насовсем" })).toBeInTheDocument();
  });

  it("says «меньше минуты» rather than counting down to zero", () => {
    render(
      <ul>
        <NoteCard
          note={trashed}
          busy={false}
          onOpen={vi.fn()}
          actions={{ kind: "trash", minutesLeft: 0, onRestore: vi.fn(), onPurge: vi.fn() }}
        />
      </ul>,
    );

    expect(screen.getByText("Осталось меньше минуты")).toBeInTheDocument();
  });

  // Erasing for good is not something a thumb should be able to do by accident,
  // and there is nothing reversible left for the other direction to offer.
  it("does not slide", () => {
    const onRestore = vi.fn();
    const onPurge = vi.fn();
    render(
      <ul>
        <NoteCard
          note={trashed}
          busy={false}
          onOpen={vi.fn()}
          actions={{ kind: "trash", minutesLeft: 30, onRestore, onPurge }}
        />
      </ul>,
    );

    const card = cardOf();
    swipe(card, 140);
    swipe(card, -140);

    expect(onRestore).not.toHaveBeenCalled();
    expect(onPurge).not.toHaveBeenCalled();
  });
});
