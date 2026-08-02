import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Check } from "lucide-react";

import { appInfo } from "@/features/notes/api";
import { describeError } from "@/shared/api/errors";
import { useT } from "@/shared/i18n";
import { APP_THEMES } from "@/shared/lib/theme";
import { useBackGuard } from "@/shared/lib/useBackGuard";
import { useTheme } from "@/shared/lib/useTheme";

/**
 * Build details, at the foot of the last screen.
 *
 * One dim line rather than a table of rows: this is the answer to "which build
 * am I looking at" when something is wrong, and nothing else. While it is
 * loading there is nothing to say, so it renders nothing instead of a spinner —
 * a footer that reserves space for a message it may never show is worse than a
 * footer that simply appears.
 */
function BuildInfo(): React.JSX.Element | null {
  const t = useT();
  const { data, error } = useQuery({ queryKey: ["app-info"], queryFn: appInfo });

  if (error !== null) {
    // A core that cannot answer is worth saying out loud, quietly.
    return <p className="text-danger mt-auto pt-6 text-center text-xs">{describeError(error, t)}</p>;
  }
  if (data === undefined) {
    return null;
  }

  return (
    <p className="text-content-muted/70 mt-auto pt-6 text-center text-xs">
      {t("settings.build", {
        name: data.name,
        version: data.version,
        schema: data.schemaVersion,
        platform: data.platform,
        count: data.noteCount,
      })}
    </p>
  );
}

/**
 * Settings.
 *
 * The theme lives in `localStorage` rather than the SQLite settings table: it is
 * a property of this install's UI, not of the notes, so it does not need to be
 * part of anything the core can read, migrate, or sync.
 *
 * Selecting a theme only flips an attribute on `<html>`, so every screen
 * restyles from CSS without React re-rendering anything.
 */
export function SettingsPage({ onBack }: { readonly onBack: () => void }): React.JSX.Element {
  const [theme, choose] = useTheme();
  const t = useT();

  useBackGuard(true, onBack);

  return (
    <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-6 p-4">
      <header className="flex items-center gap-1">
        <button
          type="button"
          aria-label={t("common.back")}
          onClick={onBack}
          className="text-content flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <ArrowLeft className="size-5" />
        </button>
        <h1 className="text-2xl font-semibold tracking-tight">{t("settings.title")}</h1>
      </header>

      <section className="flex flex-col gap-3">
        <h2 className="text-content-muted text-sm font-medium">{t("theme.appearance")}</h2>
        <div role="radiogroup" aria-label={t("theme.title")} className="grid grid-cols-2 gap-3">
          {APP_THEMES.map((option) => {
            const selected = option.id === theme;
            return (
              <button
                key={option.id}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => {
                  choose(option.id);
                }}
                className={`relative flex min-h-24 flex-col justify-end overflow-hidden rounded-2xl border p-3 text-left transition-colors ${
                  selected ? "border-accent" : "border-border-subtle"
                }`}
              >
                <span
                  aria-hidden="true"
                  className="absolute inset-0"
                  style={{ backgroundImage: option.swatch }}
                />
                {/* The scrim is what keeps the label readable on the yellow
                    preset, where the gradient itself is near-white. */}
                <span
                  aria-hidden="true"
                  className="absolute inset-0 bg-gradient-to-t from-black/55 to-transparent"
                />
                <span className="relative text-sm font-semibold text-white drop-shadow">
                  {t(option.labelKey)}
                </span>
                {selected && (
                  <span className="text-accent-content bg-accent absolute top-2 right-2 flex size-6 items-center justify-center rounded-full">
                    <Check className="size-4" />
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </section>

      <BuildInfo />
    </main>
  );
}
