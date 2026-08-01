import type { Editor } from "@tiptap/react";
import { useEditorState } from "@tiptap/react";
import { Bold, Heading2, Heading3, Italic, List, ListOrdered } from "lucide-react";

/**
 * The formatting bar.
 *
 * Active state is read through `useEditorState` rather than from the editor
 * directly: Tiptap does not re-render React on every selection change, so a
 * plain `editor.isActive()` call in the body would light the wrong buttons as
 * the caret moves.
 */
export function FormatToolbar({ editor }: { readonly editor: Editor }): React.JSX.Element {
  const state = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      h2: current.isActive("heading", { level: 2 }),
      h3: current.isActive("heading", { level: 3 }),
      bullet: current.isActive("bulletList"),
      ordered: current.isActive("orderedList"),
    }),
  });

  const actions = [
    {
      key: "bold",
      label: "Полужирный",
      icon: Bold,
      active: state.bold,
      run: () => editor.chain().focus().toggleBold().run(),
    },
    {
      key: "italic",
      label: "Курсив",
      icon: Italic,
      active: state.italic,
      run: () => editor.chain().focus().toggleItalic().run(),
    },
    {
      key: "h2",
      label: "Заголовок 2",
      icon: Heading2,
      active: state.h2,
      run: () => editor.chain().focus().toggleHeading({ level: 2 }).run(),
    },
    {
      key: "h3",
      label: "Заголовок 3",
      icon: Heading3,
      active: state.h3,
      run: () => editor.chain().focus().toggleHeading({ level: 3 }).run(),
    },
    {
      key: "bullet",
      label: "Список",
      icon: List,
      active: state.bullet,
      run: () => editor.chain().focus().toggleBulletList().run(),
    },
    {
      key: "ordered",
      label: "Нумерованный список",
      icon: ListOrdered,
      active: state.ordered,
      run: () => editor.chain().focus().toggleOrderedList().run(),
    },
  ] as const;

  return (
    <div className="bg-surface-sunken/95 border-border-subtle flex items-center gap-1 rounded-2xl border p-1 backdrop-blur">
      {actions.map(({ key, label, icon: Icon, active, run }) => (
        <button
          key={key}
          type="button"
          aria-label={label}
          aria-pressed={active}
          // The caret must survive the tap, so the button never takes focus.
          onMouseDown={(event) => {
            event.preventDefault();
          }}
          onClick={run}
          className={`flex h-11 flex-1 items-center justify-center rounded-xl transition-colors ${
            active ? "bg-accent text-accent-content" : "text-content-muted"
          }`}
        >
          <Icon className="size-5" />
        </button>
      ))}
    </div>
  );
}
