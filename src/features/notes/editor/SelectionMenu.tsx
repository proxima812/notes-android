import type { Editor } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";

import { FORMAT_BUTTON, useFormatActions, type FormatActionKey } from "./formatActions";

/**
 * What is worth offering over a selection: the marks and the block shapes that
 * are normally applied to text one has just picked out. The list toggles stay
 * out — turning a highlighted phrase into a list is a whole-paragraph decision
 * and belongs to the bar, and five buttons is what fits above a selection on a
 * phone without covering the words being formatted.
 */
const SELECTION_ACTIONS: readonly FormatActionKey[] = ["bold", "italic", "h2", "h3", "quote"];

export function SelectionMenu({ editor }: { readonly editor: Editor }): React.JSX.Element {
  const actions = useFormatActions(editor).filter((action) =>
    SELECTION_ACTIONS.includes(action.key),
  );

  return (
    <BubbleMenu
      editor={editor}
      options={{ placement: "top", offset: 10, flip: true, shift: true }}
      // Android keeps a collapsed selection alive while the caret sits in the
      // text, so an empty range must not be enough to open the menu.
      shouldShow={({ editor: current, from, to }) =>
        current.isEditable && from !== to && !current.state.selection.empty
      }
      className="bg-surface-raised border-border-subtle z-40 flex items-center gap-1 rounded-2xl border p-1 shadow-lg"
    >
      {actions.map(({ key, label, icon: Icon, active, run }) => (
        <button
          key={key}
          type="button"
          aria-label={label}
          aria-pressed={active}
          // Taking focus here would drop the very selection being formatted.
          onMouseDown={(event) => {
            event.preventDefault();
          }}
          onClick={run}
          className={`${FORMAT_BUTTON} w-11 shrink-0 ${
            active ? "bg-accent text-accent-content" : "text-content-muted"
          }`}
        >
          <Icon className="size-5" />
        </button>
      ))}
    </BubbleMenu>
  );
}
