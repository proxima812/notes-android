import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { mergeAttributes } from "@tiptap/react";
import { EditorContent, useEditor, type Editor, type JSONContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";

import { useT } from "@/shared/i18n";

import { FormatToolbar } from "./FormatToolbar";
import { LinkPreviews } from "./LinkPreviewsExtension";
import { SelectionMenu } from "./SelectionMenu";

/**
 * The link mark. Deliberately plain.
 *
 * The icon used to be chosen here, from the href, while the mark rendered. That
 * worked until an icon could arrive *after* the link was already on screen:
 * nothing about the document changes when an answer comes back from the
 * network, and ProseMirror had no reason to ask the mark to draw itself again —
 * so the first icon it picked was the one that stayed. The icon is a decoration
 * now (`LinkPreviewsExtension`), which is the mechanism that does update.
 */
const LinkWithIcon = Link.extend({
  renderHTML({ HTMLAttributes }) {
    return ["a", mergeAttributes(this.options.HTMLAttributes, HTMLAttributes), 0];
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
      LinkPreviews,
    ],
    // The editor is built in an effect, not during render. The screen arrives
    // through `lazy` + `Suspense`, and the first open is the one render React
    // throws away and repeats; a ProseMirror view created during that discarded
    // pass is the empty body people see until they leave and come back.
    immediatelyRender: false,
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
      {/* Stuck below the status bar, not below the top of the window: `top: 0`
          is measured from the viewport, which on an edge-to-edge screen starts
          behind the clock and the battery. */}
      <div className="sticky top-[env(safe-area-inset-top)] z-10 pt-1">
        <FormatToolbar editor={editor} />
      </div>
      <SelectionMenu editor={editor} />
      <EditorContent editor={editor} className="flex-1" />
    </div>
  );
}
