import type { Editor } from "@tiptap/react";
import { useEditorState } from "@tiptap/react";
import { useT } from "@/shared/i18n";
import {
  Bold,
  Heading2,
  Heading3,
  Italic,
  List,
  ListOrdered,
  TextQuote,
  type LucideIcon,
} from "lucide-react";

/*
 * The set is deliberately one family: `B` and `I` are letterforms because that
 * is what a text tool is expected to show, and everything else is the "lines of
 * text" glyph — headings, quote, both lists. `Quote` used to sit here and drew a
 * pair of oversized quotation marks: correct as a symbol, but a full stop of ink
 * next to six light line drawings.
 */

export type FormatActionKey = "bold" | "italic" | "h2" | "h3" | "quote" | "bullet" | "ordered";

export interface FormatAction {
  readonly key: FormatActionKey;
  readonly label: string;
  readonly icon: LucideIcon;
  readonly active: boolean;
  readonly run: () => void;
}

/**
 * One definition of what the editor can do, so the always-visible bar and the
 * menu that pops up over a selection cannot disagree about an icon, a label or
 * what counts as active.
 *
 * Active state is read through `useEditorState` rather than off the editor
 * directly: Tiptap does not re-render React on every selection change, so a
 * plain `editor.isActive()` call in a component body would light the wrong
 * buttons as the caret moves.
 */
export function useFormatActions(editor: Editor): readonly FormatAction[] {
  const t = useT();
  const state = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      h2: current.isActive("heading", { level: 2 }),
      h3: current.isActive("heading", { level: 3 }),
      quote: current.isActive("blockquote"),
      bullet: current.isActive("bulletList"),
      ordered: current.isActive("orderedList"),
    }),
  });

  return [
    {
      key: "bold",
      label: t("format.bold"),
      icon: Bold,
      active: state.bold,
      run: () => editor.chain().focus().toggleBold().run(),
    },
    {
      key: "italic",
      label: t("format.italic"),
      icon: Italic,
      active: state.italic,
      run: () => editor.chain().focus().toggleItalic().run(),
    },
    {
      key: "h2",
      label: t("format.h2"),
      icon: Heading2,
      active: state.h2,
      run: () => editor.chain().focus().toggleHeading({ level: 2 }).run(),
    },
    {
      key: "h3",
      label: t("format.h3"),
      icon: Heading3,
      active: state.h3,
      run: () => editor.chain().focus().toggleHeading({ level: 3 }).run(),
    },
    {
      key: "quote",
      label: t("format.quote"),
      icon: TextQuote,
      active: state.quote,
      run: () => editor.chain().focus().toggleBlockquote().run(),
    },
    {
      key: "bullet",
      label: t("format.bullet"),
      icon: List,
      active: state.bullet,
      run: () => editor.chain().focus().toggleBulletList().run(),
    },
    {
      key: "ordered",
      label: t("format.ordered"),
      icon: ListOrdered,
      active: state.ordered,
      run: () => editor.chain().focus().toggleOrderedList().run(),
    },
  ];
}

/** The button itself is identical in both menus; only the width rule differs. */
export const FORMAT_BUTTON =
  "flex h-11 items-center justify-center rounded-xl transition-colors";
