import { ChevronDown } from "lucide-react";

/**
 * Shared classes for every control that opens a picker: a date, a time, a list
 * of choices, or a button standing in for a sheet.
 *
 * The right padding is what leaves room for the arrow drawn over it.
 */
export const PICKER_CLASSES =
  "picker-field bg-surface-raised text-content focus:ring-accent min-h-11 w-full rounded-xl pl-3 pr-10 outline-none focus:ring-2";

/**
 * Wraps a picker control and draws its arrow.
 *
 * The arrow is ours rather than the browser's because Chromium pins the native
 * one to the very edge of the field — see `.picker-field` in `global.css`. It
 * ignores taps so that hitting it still opens the picker underneath.
 *
 * Every field that opens something wears this, so that a date, a time and a
 * list of choices read as the same kind of control wherever they appear.
 */
export function PickerField({
  children,
  className = "",
}: {
  readonly children: React.ReactNode;
  readonly className?: string;
}): React.JSX.Element {
  return (
    <div className={`relative ${className}`}>
      {children}
      <ChevronDown className="text-content-muted pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2" />
    </div>
  );
}
