import { describe, expect, it } from "vitest";

import { ru } from "@/shared/i18n/locales/ru";
import type { Translate } from "@/shared/i18n";

import { noteTemplates, buildDoc, buildText } from "./templates";

/** The dictionary itself, so the fixtures read as the Russian strings they are. */
const t: Translate = (key) => ru[key];

const NOTE_TEMPLATES = noteTemplates(t);

describe("note templates", () => {
  it("ships a grocery template grouped by aisle", () => {
    const groceries = NOTE_TEMPLATES.find((template) => template.id === "groceries");

    expect(groceries?.noteType).toBe("shopping_list");
    expect(buildText(groceries!)).toContain("Овощи и фрукты");
  });

  it("builds documents the editor's schema accepts", () => {
    for (const template of NOTE_TEMPLATES) {
      const doc = JSON.parse(buildDoc(template)) as {
        type: string;
        content: { type: string; attrs?: { level?: number } }[];
      };

      expect(doc.type).toBe("doc");
      expect(doc.content.length).toBeGreaterThan(0);
      for (const node of doc.content) {
        expect(["heading", "paragraph", "bulletList", "orderedList"]).toContain(node.type);
        // The toolbar only offers H2/H3, so a template must not introduce a
        // heading level the editor cannot round-trip.
        if (node.type === "heading") {
          expect([2, 3]).toContain(node.attrs?.level);
        }
      }
    }
  });

  it("keeps the text projection line-for-line with the document blocks", () => {
    for (const template of NOTE_TEMPLATES) {
      const doc = JSON.parse(buildDoc(template)) as {
        content: { type: string; content?: unknown[] }[];
      };
      const blocks = doc.content.reduce(
        (total, node) =>
          node.type === "bulletList" || node.type === "orderedList"
            ? total + (node.content?.length ?? 0)
            : total + 1,
        0,
      );

      expect(buildText(template).split("\n")).toHaveLength(blocks);
    }
  });

  it("gives every template a distinct id and a non-empty title", () => {
    const ids = NOTE_TEMPLATES.map((template) => template.id);

    expect(new Set(ids).size).toBe(ids.length);
    for (const template of NOTE_TEMPLATES) {
      expect(template.title).not.toBe("");
      expect(template.label).not.toBe("");
    }
  });
});
