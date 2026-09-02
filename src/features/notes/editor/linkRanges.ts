/**
 * Where the links are in a document, and whether one still shows its address.
 *
 * Kept apart from the extension so the two rules that matter — what counts as
 * one link, and what counts as a link that has not been renamed yet — can be
 * tested without a ProseMirror view.
 */

import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

/** One run of text carrying the same link mark. */
export interface LinkRange {
  readonly from: number;
  readonly to: number;
  readonly href: string;
  readonly text: string;
}

/** The address as a person reads it: no scheme, no `www.`, no trailing slash. */
function bare(address: string): string {
  return address
    .trim()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/^www\./i, "")
    .replace(/\/+$/, "")
    .toLowerCase();
}

/**
 * Whether a link is still showing its own address rather than a name.
 *
 * This is the whole guard on replacing text: a link somebody typed a name for,
 * or already renamed, must never be rewritten under them — only the URL that
 * autolink or a paste just dropped in is fair game.
 */
export function showsItsOwnAddress(text: string, href: string): boolean {
  const shown = bare(text);
  return shown !== "" && shown === bare(href);
}

/**
 * Groups the text nodes of a document into one entry per link.
 *
 * A link split across several text nodes — half of it bold, or with the caret
 * having once sat inside it — is one link, so adjacent nodes with the same href
 * are merged. Anything else would rename half a link and leave the rest.
 */
export function linkRangesIn(doc: ProseMirrorNode): LinkRange[] {
  const ranges: LinkRange[] = [];

  doc.descendants((node, position) => {
    if (!node.isText || node.text === undefined) {
      return;
    }
    const mark = node.marks.find((candidate) => candidate.type.name === "link");
    if (mark === undefined) {
      return;
    }
    const href = typeof mark.attrs["href"] === "string" ? mark.attrs["href"] : "";
    if (href === "") {
      return;
    }

    const last = ranges.at(-1);
    if (last !== undefined && last.to === position && last.href === href) {
      const text = node.text;
      ranges[ranges.length - 1] = {
        ...last,
        to: position + text.length,
        text: last.text + text,
      };
      return;
    }
    ranges.push({
      from: position,
      to: position + node.text.length,
      href,
      text: node.text,
    });
  });

  return ranges;
}
