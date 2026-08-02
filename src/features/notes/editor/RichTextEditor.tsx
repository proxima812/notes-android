import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { mergeAttributes } from "@tiptap/react";
import { EditorContent, useEditor, type Editor, type JSONContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";

import { useT } from "@/shared/i18n";

import { FormatToolbar } from "./FormatToolbar";
import { siteOf } from "./linkSites";
import { SelectionMenu } from "./SelectionMenu";

/**
 * The link mark, tagged with the service it points at.
 *
 * `data-site` is computed while rendering rather than stored as an attribute:
 * the icon is a presentation detail, and baking it into `content_json` would
 * freeze today's mapping into every note ever saved.
 *
 * The icon is drawn by CSS as a `::before`, not as a DOM child, because anything
 * inside the anchor would be editable content the caret could land in.
 */
const LinkWithIcon = Link.extend({
  renderHTML({ HTMLAttributes }) {
    const site = siteOf(HTMLAttributes["href"]);
    return [
      "a",
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        ...(site === null ? {} : { "data-site": site }),
      }),
      0,
    ];
  },
});

export interface EditorSnapshot {
  /** Tiptap document, serialised. Stored verbatim in `notes.content_json`. */
  readonly contentJson: string;
  /** Plain-text projection. This is what FTS5 indexes, so it must stay in sync. */
  readonly contentText: string;
}

interface RichTextEditorProps {
  /** Serialised Tiptap document, or `null` for a note written before the editor. */
  readonly initialJson: string | null;
  /** Used when there is no document yet, so existing plain notes open intact. */
  readonly initialText: string;
  readonly onChange: (snapshot: EditorSnapshot) => void;
}

/**
 * Parses a stored document, falling back to the plain text projection.
 *
 * A note may predate the editor, and `content_json` is opaque to SQLite, so a
 * malformed document has to degrade to readable text instead of an empty screen.
 */
function initialContent(json: string | null, text: string): JSONContent | string {
  if (json === null || json === "") {
    return text;
  }
  try {
    return JSON.parse(json) as JSONContent;
  } catch {
    return text;
  }
}

function snapshotOf(editor: Editor): EditorSnapshot {
  return {
    contentJson: JSON.stringify(editor.getJSON()),
    // A newline between blocks keeps headings and list items on separate lines
    // in the preview and gives FTS5 a token boundary between them.
    contentText: editor.getText({ blockSeparator: "\n" }),
  };
}

export function RichTextEditor({
  initialJson,
  initialText,
  onChange,
}: RichTextEditorProps): React.JSX.Element {
  const t = useT();
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        // Only the two levels the toolbar offers; a phone screen has no room
        // for a six-level hierarchy and H1 competes with the note title.
        heading: { levels: [2, 3] },
        // Replaced below by the variant that tags the service.
        link: false,
      }),
      LinkWithIcon.configure({
        // `autolink` formats a URL as it is typed, `linkOnPaste` catches the far
        // more common case on a phone: pasting one in from another app.
        autolink: true,
        linkOnPaste: true,
        defaultProtocol: "https",
        // Tapping must not navigate: this WebView *is* the app, and following a
        // link inside it would replace the note with a web page and no way back.
        openOnClick: false,
        protocols: ["http", "https", "mailto", "tel"],
      }),
      Placeholder.configure({ placeholder: t("editor.bodyPlaceholder") }),
    ],
    content: initialContent(initialJson, initialText),
    editorProps: {
      attributes: {
        class: "note-body min-h-64 outline-none",
      },
    },
    onUpdate: ({ editor: current }) => {
      onChange(snapshotOf(current));
    },
  });

  if (editor === null) {
    return <div className="min-h-64" />;
  }

  return (
    <div className="flex flex-1 flex-col gap-3">
      <div className="sticky top-0 z-10 pt-1">
        <FormatToolbar editor={editor} />
      </div>
      <SelectionMenu editor={editor} />
      <EditorContent editor={editor} className="flex-1" />
    </div>
  );
}
