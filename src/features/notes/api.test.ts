import { describe, expect, it } from "vitest";

import { HIGHLIGHT_CLOSE, HIGHLIGHT_OPEN, splitHighlights } from "./api";

const wrap = (text: string): string => `${HIGHLIGHT_OPEN}${text}${HIGHLIGHT_CLOSE}`;

describe("splitHighlights", () => {
  it("returns a single plain run when there is nothing to highlight", () => {
    expect(splitHighlights("молоко и хлеб")).toEqual([
      { text: "молоко и хлеб", highlighted: false },
    ]);
  });

  it("splits a match out of the surrounding text", () => {
    expect(splitHighlights(`купить ${wrap("молоко")} сегодня`)).toEqual([
      { text: "купить ", highlighted: false },
      { text: "молоко", highlighted: true },
      { text: " сегодня", highlighted: false },
    ]);
  });

  it("handles a match at the very start", () => {
    expect(splitHighlights(`${wrap("молоко")} сегодня`)).toEqual([
      { text: "молоко", highlighted: true },
      { text: " сегодня", highlighted: false },
    ]);
  });

  it("handles a match at the very end", () => {
    expect(splitHighlights(`купить ${wrap("молоко")}`)).toEqual([
      { text: "купить ", highlighted: false },
      { text: "молоко", highlighted: true },
    ]);
  });

  it("handles several matches", () => {
    expect(splitHighlights(`${wrap("молоко")} и ${wrap("хлеб")}`)).toEqual([
      { text: "молоко", highlighted: true },
      { text: " и ", highlighted: false },
      { text: "хлеб", highlighted: true },
    ]);
  });

  it("returns nothing for an empty snippet", () => {
    expect(splitHighlights("")).toEqual([]);
  });

  it("does not lose text when a closing marker is missing", () => {
    // Defensive: a truncated snippet must still render its words.
    const parts = splitHighlights(`купить ${HIGHLIGHT_OPEN}молоко`);
    expect(parts.map((part) => part.text).join("")).toBe("купить молоко");
  });

  it("treats text that merely looks like markup as plain text", () => {
    const parts = splitHighlights("см. [1] и [2]");
    expect(parts).toEqual([{ text: "см. [1] и [2]", highlighted: false }]);
  });
});
