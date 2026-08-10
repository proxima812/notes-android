import { Palette } from "lucide-react";
import { useState } from "react";

import { APP_THEMES } from "@/shared/lib/theme";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import { useT } from "@/shared/i18n";
import { useTheme } from "@/shared/lib/useTheme";
import { ACTION_BUTTON_SECONDARY } from "@/shared/ui/actionButton";

/**
 * The quick theme control that sits next to the compose button.
 *
 * Swatches open in a row hanging off the button rather than as a sheet:
 * switching themes is something people do repeatedly to compare, and a sheet
 * would cover the very screen being judged. The row is anchored to the right
 * edge because the button is the last in its line and a left-anchored panel
 * would run off the screen.
 */
export function ThemeSwitcher(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const [theme, choose] = useTheme();
  const t = useT();
  const current = APP_THEMES.find((option) => option.id === theme) ?? APP_THEMES[0];

  useBackGuard(open, () => {
    setOpen(false);
  });

  return (
    <div className="relative">
      {open && (
        <>
          <button
            type="button"
            aria-label={t("theme.close")}
            onClick={() => {
              setOpen(false);
            }}
            className="fixed inset-0 z-20 cursor-default"
          />
          <div
            role="radiogroup"
            aria-label={t("theme.title")}
            className="bg-surface-raised border-border-subtle absolute top-full right-0 z-30 mt-2 flex gap-2 rounded-2xl border p-2 shadow-lg"
          >
            {APP_THEMES.map((option) => (
              <button
                key={option.id}
                type="button"
                role="radio"
                aria-checked={option.id === theme}
                aria-label={t(option.labelKey)}
                title={t(option.labelKey)}
                onClick={() => {
                  choose(option.id);
                }}
                style={{ backgroundImage: option.swatch }}
                className={`size-11 rounded-full border-2 transition-colors ${
                  option.id === theme ? "border-accent" : "border-border-subtle"
                }`}
              />
            ))}
          </div>
        </>
      )}

      <button
        type="button"
        aria-label={t("theme.title")}
        aria-expanded={open}
        onClick={() => {
          setOpen((shown) => !shown);
        }}
        className={`${ACTION_BUTTON_SECONDARY} relative`}
      >
        <Palette className="size-5" />
        {/* The current theme is stated by a dot rather than by painting the whole
            button: a full gradient made this control the loudest thing on the
            screen and left no readable colour for the icon on the pale presets. */}
        <span
          aria-hidden="true"
          style={{ backgroundImage: current?.swatch }}
          // The outline is what keeps the near-black Обсидиан swatch from
          // disappearing into the button it sits on.
          className="border-border-subtle absolute top-1.5 right-1.5 size-2.5 rounded-full border"
        />
      </button>
    </div>
  );
}
