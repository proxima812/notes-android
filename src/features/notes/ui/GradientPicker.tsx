import { Check, Ban } from "lucide-react";

import { useT } from "@/shared/i18n";
import { NOTE_GRADIENTS } from "@/shared/lib/gradients";

interface GradientPickerProps {
  readonly value: string | null;
  readonly onChange: (color: string | null) => void;
}

export function GradientPicker({ value, onChange }: GradientPickerProps): React.JSX.Element {
  const t = useT();

  return (
    <div
      role="radiogroup"
      aria-label={t("editor.color")}
      className="-mx-4 flex gap-2 overflow-x-auto px-4 pb-1"
    >
      <button
        type="button"
        role="radio"
        aria-checked={value === null}
        aria-label={t("editor.noColor")}
        onClick={() => {
          onChange(null);
        }}
        className="border-border-subtle text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full border"
      >
        {value === null ? <Check className="size-5" /> : <Ban className="size-4" />}
      </button>

      {NOTE_GRADIENTS.map((gradient) => (
        <button
          key={gradient.id}
          type="button"
          role="radio"
          aria-checked={value === gradient.id}
          aria-label={t(gradient.labelKey)}
          onClick={() => {
            onChange(gradient.id);
          }}
          style={{ backgroundImage: gradient.swatch }}
          className="flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          {value === gradient.id && (
            <Check className="size-5 text-black/70" strokeWidth={3} aria-hidden />
          )}
        </button>
      ))}
    </div>
  );
}
