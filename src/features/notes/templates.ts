import type { StringKey, Translate } from "@/shared/i18n";

import type { NoteType } from "./api";

/**
 * Note templates.
 *
 * A template is declared as structure, not as a Tiptap document plus a matching
 * string: `buildDoc` and `buildText` are both derived from the same blocks, so
 * the stored document and the plain-text projection FTS5 indexes cannot drift
 * apart the way two hand-written copies would.
 *
 * Only what `StarterKit` is configured with is used here — H2/H3, paragraphs and
 * lists. There is no checkbox node in this build, so checklists are bullets.
 *
 * Templates are declared as dictionary keys and resolved through `noteTemplates(t)`
 * at the moment of use, because a template is not chrome: its headings are typed
 * into the note and saved. Building them at call time is what makes a note
 * created in Kazakh actually arrive in Kazakh instead of in the language the app
 * happened to start in.
 */

export interface TemplateBlock {
  readonly heading?: string;
  readonly paragraph?: string;
  readonly bullets?: readonly string[];
  readonly ordered?: readonly string[];
}

interface TemplateBlockSpec {
  readonly heading?: StringKey;
  readonly paragraph?: StringKey;
  /** An empty paragraph: room to write, rather than a line of prompt text. */
  readonly blank?: true;
  /** Empty lines to leave for the reader; only the count matters. */
  readonly bullets?: number;
  readonly ordered?: number;
}

interface NoteTemplateSpec {
  readonly id: string;
  readonly labelKey: StringKey;
  readonly hintKey: StringKey;
  readonly noteType: NoteType;
  readonly blocks: readonly TemplateBlockSpec[];
}

export interface NoteTemplate {
  readonly id: string;
  readonly label: string;
  readonly hint: string;
  readonly noteType: NoteType;
  readonly title: string;
  readonly blocks: readonly TemplateBlock[];
}

const TEMPLATE_SPECS: readonly NoteTemplateSpec[] = [
  {
    id: "groceries",
    labelKey: "template.groceries.label",
    hintKey: "template.groceries.hint",
    noteType: "shopping_list",
    blocks: [
      { heading: "template.groceries.produce", bullets: 3 },
      { heading: "template.groceries.dairy", bullets: 2 },
      { heading: "template.groceries.meat", bullets: 2 },
      { heading: "template.groceries.pantry", bullets: 3 },
      { heading: "template.groceries.household", bullets: 2 },
      { heading: "template.groceries.budget", paragraph: "template.groceries.budgetHint" },
    ],
  },
  {
    id: "day",
    labelKey: "template.day.label",
    hintKey: "template.day.hint",
    noteType: "daily_note",
    blocks: [
      { heading: "template.day.focus", blank: true },
      { heading: "template.day.tasks", bullets: 3 },
      { heading: "template.day.meetings", bullets: 2 },
      { heading: "template.day.notes", blank: true },
    ],
  },
  {
    id: "meeting",
    labelKey: "template.meeting.label",
    hintKey: "template.meeting.hint",
    noteType: "meeting",
    blocks: [
      { heading: "template.meeting.people", bullets: 2 },
      { heading: "template.meeting.agenda", ordered: 3 },
      { heading: "template.meeting.decisions", bullets: 2 },
      { heading: "template.meeting.tasks", bullets: 2 },
    ],
  },
  {
    id: "trip",
    labelKey: "template.trip.label",
    hintKey: "template.trip.hint",
    noteType: "checklist",
    blocks: [
      { heading: "template.trip.documents", bullets: 2 },
      { heading: "template.trip.clothes", bullets: 3 },
      { heading: "template.trip.tech", bullets: 2 },
      { heading: "template.trip.meds", bullets: 2 },
      { heading: "template.trip.before", bullets: 2 },
    ],
  },
  {
    id: "recipe",
    labelKey: "template.recipe.label",
    hintKey: "template.recipe.hint",
    noteType: "text",
    blocks: [
      { heading: "template.recipe.ingredients", bullets: 4 },
      { heading: "template.recipe.steps", ordered: 3 },
      { heading: "template.recipe.notes", blank: true },
    ],
  },
  {
    id: "project",
    labelKey: "template.project.label",
    hintKey: "template.project.hint",
    noteType: "project",
    blocks: [
      { heading: "template.project.goal", blank: true },
      { heading: "template.project.steps", ordered: 3 },
      { heading: "template.project.risks", bullets: 2 },
      { heading: "template.project.links", bullets: 1 },
    ],
  },
];

/** Resolves the specs into the current language. The title is the label. */
export function noteTemplates(t: Translate): readonly NoteTemplate[] {
  return TEMPLATE_SPECS.map((spec) => ({
    id: spec.id,
    label: t(spec.labelKey),
    hint: t(spec.hintKey),
    noteType: spec.noteType,
    title: t(spec.labelKey),
    blocks: spec.blocks.map((block) => ({
      ...(block.heading === undefined ? {} : { heading: t(block.heading) }),
      ...(block.paragraph === undefined ? {} : { paragraph: t(block.paragraph) }),
      ...(block.blank === true ? { paragraph: "" } : {}),
      ...(block.bullets === undefined ? {} : { bullets: Array<string>(block.bullets).fill("") }),
      ...(block.ordered === undefined ? {} : { ordered: Array<string>(block.ordered).fill("") }),
    })),
  }));
}

interface DocNode {
  readonly type: string;
  readonly attrs?: Readonly<Record<string, unknown>>;
  readonly text?: string;
  readonly content?: readonly DocNode[];
}

function paragraph(text: string): DocNode {
  return text === ""
    ? { type: "paragraph" }
    : { type: "paragraph", content: [{ type: "text", text }] };
}

function list(type: "bulletList" | "orderedList", items: readonly string[]): DocNode {
  return {
    type,
    content: items.map((item) => ({ type: "listItem", content: [paragraph(item)] })),
  };
}

function blockNodes(block: TemplateBlock): readonly DocNode[] {
  const nodes: DocNode[] = [];
  if (block.heading !== undefined) {
    nodes.push({
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: block.heading }],
    });
  }
  if (block.paragraph !== undefined) {
    nodes.push(paragraph(block.paragraph));
  }
  if (block.bullets !== undefined) {
    nodes.push(list("bulletList", block.bullets));
  }
  if (block.ordered !== undefined) {
    nodes.push(list("orderedList", block.ordered));
  }
  return nodes;
}

/** The Tiptap document, serialised exactly as the editor would store it. */
export function buildDoc(template: NoteTemplate): string {
  return JSON.stringify({
    type: "doc",
    content: template.blocks.flatMap(blockNodes),
  });
}

/**
 * The plain-text projection, mirroring `editor.getText({ blockSeparator: "\n" })`:
 * one line per block, empty blocks included, so the two stay comparable.
 */
export function buildText(template: NoteTemplate): string {
  const lines: string[] = [];
  for (const block of template.blocks) {
    if (block.heading !== undefined) {
      lines.push(block.heading);
    }
    if (block.paragraph !== undefined) {
      lines.push(block.paragraph);
    }
    lines.push(...(block.bullets ?? []), ...(block.ordered ?? []));
  }
  return lines.join("\n");
}
