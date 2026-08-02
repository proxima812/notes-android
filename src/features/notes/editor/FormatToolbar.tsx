import type { Editor } from "@tiptap/react";

import { FORMAT_BUTTON, useFormatActions } from "./formatActions";

/**
 * The formatting bar above the note body.
 *
 * Every button is one equal share of the row (`flex-1 basis-0`) rather than
 * sized to its icon: the glyphs differ in width — `B` against a numbered-list
 * icon — and hit targets that follow the glyph make the bar look ragged and
 * misaddress taps near the edges.
 */
export function FormatToolbar({ editor }: { readonly editor: Editor }): React.JSX.Element {
  const actions = useFormatActions(editor);

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
          className={`${FORMAT_BUTTON} min-w-0 flex-1 basis-0 ${
            active ? "bg-accent text-accent-content" : "text-content-muted"
          }`}
        >
          <Icon className="size-5" />
        </button>
      ))}
    </div>
  );
}
