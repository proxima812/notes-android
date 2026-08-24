import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, ChevronRight } from "lucide-react";
import { useState } from "react";

import { BackupSection } from "@/features/backup/ui/BackupSection";
import { appInfo } from "@/features/notes/api";
import { QuickNoteSettingsSection } from "@/features/quick-notes/ui/QuickNoteSettingsSection";
import { SnoozeSettingSection } from "@/features/reminders/ui/SnoozeSettingSection";
import { LanguagePicker } from "@/features/settings/ui/LanguagePicker";
import { describeError } from "@/shared/api/errors";
import { LANGUAGES, useLanguage, useT } from "@/shared/i18n";
import {
  DEFAULT_APP_NAME,
  MAX_APP_NAME_LENGTH,
  limitAppName,
  loadStoredAppName,
} from "@/shared/lib/appName";
import { APP_THEMES } from "@/shared/lib/theme";
import { useAppName } from "@/shared/lib/useAppName";
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
 * The name on the library header, and the field that changes it.
 *
 * The field holds its own draft rather than reading the displayed name back:
 * emptying it means "go back to the default", and a field that answered that by
 * filling itself with `xima.keeps` would fight anyone trying to clear it before
 * typing. So an empty field shows the default as a placeholder and the header
 * falls back to it, which is the same thing said twice in the right places.
 *
 * Every keystroke is saved. There is no confirm button because there is nothing
 * to confirm: the header behind this screen is already showing the result.
 */
function AppNameSection(): React.JSX.Element {
  const t = useT();
  const [, save] = useAppName();
  const [draft, setDraft] = useState(() => loadStoredAppName() ?? "");

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-content-muted text-sm font-medium">{t("appName.title")}</h2>
      <input
        type="text"
        value={draft}
        // The cap is enforced on the way in as well: `maxLength` counts UTF-16
        // units, so it alone would let ten emoji through as twenty.
        maxLength={MAX_APP_NAME_LENGTH}
        placeholder={DEFAULT_APP_NAME}
        aria-label={t("appName.title")}
        onChange={(event) => {
          // The draft keeps its spaces so a name can be typed in two words;
          // storage trims them, so the header never shows a name that starts
          // half a space away from the edge.
          const typed = limitAppName(event.target.value);
          setDraft(typed);
          save(typed);
        }}
        className="bg-surface-raised text-content placeholder:text-content-muted/60 focus:ring-accent min-h-11 rounded-2xl px-4 text-base outline-none focus:ring-2"
      />
      <p className="text-content-muted text-xs">
        {t("appName.hint", { max: MAX_APP_NAME_LENGTH })}
      </p>
    </section>
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
  const [language] = useLanguage();
  const [languageOpen, setLanguageOpen] = useState(false);
  const t = useT();
  const currentTheme = APP_THEMES.find((option) => option.id === theme);
  const currentLanguage = LANGUAGES.find((option) => option.id === language);

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

      <AppNameSection />

      <QuickNoteSettingsSection />

      <SnoozeSettingSection />

      {/* The palette, said as four dots.
          What this replaced was four labelled panels of gradient, which made the
          quietest setting in the app the loudest thing on the screen — and none
          of that area said anything the colour itself does not. What is left is
          the colour, a ring round the one in use, and its name written once
          underneath. */}
      <section className="flex flex-col gap-3">
        <h2 className="text-content-muted text-sm font-medium">{t("theme.appearance")}</h2>
        <div role="radiogroup" aria-label={t("theme.title")} className="flex gap-3">
          {APP_THEMES.map((option) => {
            const selected = option.id === theme;
            return (
              <button
                key={option.id}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={t(option.labelKey)}
                onClick={() => {
                  choose(option.id);
                }}
                // The ring is drawn outside the swatch with a gap, so it reads as
                // a mark of choice rather than as an edge of the colour — and so
                // the near-black Обсидиан dot is still visible when it is the one
                // chosen.
                className={`size-11 rounded-full ${
                  selected ? "ring-accent ring-offset-surface ring-2 ring-offset-2" : ""
                }`}
                style={{ backgroundImage: option.swatch }}
              />
            );
          })}
        </div>
        {currentTheme !== undefined && (
          <p className="text-content-muted text-xs">{t(currentTheme.labelKey)}</p>
        )}
      </section>

      {/* Language moved in here from the library header, where it was a third
          icon competing with search and settings. The endonym is on the row, so
          the way out of a language you cannot read is still a word you can. */}
      <section className="flex flex-col gap-3">
        <h2 className="text-content-muted text-sm font-medium">{t("language.title")}</h2>
        <button
          type="button"
          aria-expanded={languageOpen}
          onClick={() => {
            setLanguageOpen(true);
          }}
          className="bg-surface-raised text-content flex min-h-12 items-center justify-between rounded-2xl px-4 text-left"
        >
          <span className="font-medium" lang={currentLanguage?.id}>
            {currentLanguage?.label ?? ""}
          </span>
          <ChevronRight aria-hidden="true" className="text-content-muted size-5 shrink-0" />
        </button>
      </section>

      {languageOpen && (
        <LanguagePicker
          onClose={() => {
            setLanguageOpen(false);
          }}
        />
      )}

      <BackupSection />

      <BuildInfo />
    </main>
  );
}
