import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor, type Editor, type JSONContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";

import { FormatToolbar } from "./FormatToolbar";

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
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        // Only the two levels the toolbar offers; a phone screen has no room
        // for a six-level hierarchy and H1 competes with the note title.
        heading: { levels: [2, 3] },
        link: false,
      }),
      Placeholder.configure({ placeholder: "Текст заметки…" }),
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
      <EditorContent editor={editor} className="flex-1" />
      <div className="sticky bottom-0 pb-1">
        <FormatToolbar editor={editor} />
      </div>
    </div>
  );
}
