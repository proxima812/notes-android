import { useT } from "@/shared/i18n";

import { noteTemplates, type NoteTemplate } from "../templates";

/**
 * The template chooser.
 *
 * A sheet rather than a row of chips: each template needs a line of explanation
 * to be picked confidently, and six of those do not fit anywhere the compose
 * button can reach.
 */
export function TemplatePicker({
  busy,
  onPick,
  onClose,
}: {
  readonly busy: boolean;
  readonly onPick: (template: NoteTemplate) => void;
  readonly onClose: () => void;
}): React.JSX.Element {
  const t = useT();
  const templates = noteTemplates(t);

  return (
    <>
      <button
        type="button"
        aria-label={t("templates.close")}
        onClick={onClose}
        className="fixed inset-0 z-40 cursor-default bg-black/50"
      />
      <div
        role="dialog"
        aria-label={t("templates.dialog")}
        className="bg-surface-raised border-border-subtle fixed inset-x-0 bottom-0 z-50 mx-auto max-w-md rounded-t-3xl border-t p-4 pb-[calc(1rem+env(safe-area-inset-bottom))] shadow-2xl"
      >
        <div className="bg-border-subtle mx-auto mb-4 h-1 w-10 rounded-full" />
        <h2 className="mb-3 text-lg font-semibold tracking-tight">{t("templates.title")}</h2>
        <ul className="flex flex-col gap-2">
          {templates.map((template) => (
            <li key={template.id}>
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  onPick(template);
                }}
                className="bg-surface-sunken border-border-subtle flex min-h-14 w-full flex-col justify-center rounded-2xl border px-4 py-2 text-left disabled:opacity-40"
              >
                <span className="font-medium">{template.label}</span>
                <span className="text-content-muted text-sm">{template.hint}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}
