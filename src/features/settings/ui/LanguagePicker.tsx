import { Check } from "lucide-react";

import { useBackGuard } from "@/shared/lib/useBackGuard";
import { LANGUAGES, useLanguage, useT } from "@/shared/i18n";

/**
 * The language sheet.
 *
 * A sheet rather than the theme switcher's row of swatches: eight endonyms in
 * four scripts have no compact shape, and the one thing that must work here is
 * finding your own language in a list currently written in one you may not read.
 * For the same reason every row is labelled only in itself.
 */
export function LanguagePicker({ onClose }: { readonly onClose: () => void }): React.JSX.Element {
  const [language, choose] = useLanguage();
  const t = useT();

  useBackGuard(true, onClose);

  return (
    <>
      <button
        type="button"
        aria-label={t("language.close")}
        onClick={onClose}
        className="fixed inset-0 z-40 cursor-default bg-black/50"
      />
      <div
        role="radiogroup"
        aria-label={t("language.title")}
        className="bg-surface-raised fixed inset-x-0 bottom-0 z-50 mx-auto max-w-md rounded-t-3xl p-4 pb-[calc(1rem+env(safe-area-inset-bottom))] shadow-2xl"
      >
        <div className="bg-content-muted/40 mx-auto mb-4 h-1 w-10 rounded-full" />
        <h2 className="mb-3 text-lg font-semibold tracking-tight">{t("language.title")}</h2>
        <ul className="flex flex-col gap-1">
          {LANGUAGES.map((option) => {
            const selected = option.id === language;
            return (
              <li key={option.id}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  lang={option.id}
                  onClick={() => {
                    choose(option.id);
                    onClose();
                  }}
                  className={`flex min-h-12 w-full items-center justify-between rounded-2xl px-4 text-left ${
                    selected ? "bg-accent text-accent-content" : "bg-surface-sunken"
                  }`}
                >
                  <span className="font-medium">{option.label}</span>
                  {selected && <Check className="size-5 shrink-0" />}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </>
  );
}
