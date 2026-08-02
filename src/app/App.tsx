import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Languages, Settings } from "lucide-react";
import { useEffect, useState } from "react";

import { reconcileReminderZone } from "@/features/reminders/api";
import { useReminderLaunchTarget } from "@/features/reminders/useReminderLaunchTarget";
import { LanguagePicker } from "@/features/settings/ui/LanguagePicker";
import { SettingsPage } from "@/features/settings/ui/SettingsPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { NoteEditorPage } from "@/pages/NoteEditorPage";
import { I18nProvider, useT } from "@/shared/i18n";
import type { NoteId } from "@/shared/types/ids";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Everything is local; refetching on focus costs battery and buys nothing.
      refetchOnWindowFocus: false,
      staleTime: 10_000,
      retry: false,
    },
  },
});

type Route = { readonly kind: "library" } | { readonly kind: "note"; readonly id: NoteId } | { readonly kind: "settings" };

/**
 * Three screens, so the route is a piece of state rather than a router: adding a
 * dependency to model "list, one note, or settings" would be more machinery than
 * the app has decisions to make.
 */
function Shell(): React.JSX.Element {
  const [route, setRoute] = useState<Route>({ kind: "library" });
  const [languageOpen, setLanguageOpen] = useState(false);
  const t = useT();

  // A reminder means a time on a clock, so crossing a border has to move the
  // instant rather than the time. Nothing announces that the device changed
  // zone, so the question is asked once per start and is usually answered
  // "nothing moved".
  useEffect(() => {
    void reconcileReminderZone().catch(() => {
      // The app must still open. A reminder left in yesterday's zone is worth
      // less than a start screen that refuses to appear.
    });
  }, []);

  // A tapped reminder overrides whatever was on screen, including a note the
  // user had left open: they asked for this note, just now, by tapping it.
  useReminderLaunchTarget((id) => {
    setRoute({ kind: "note", id });
    setLanguageOpen(false);
  });

  const toLibrary = (): void => {
    setRoute({ kind: "library" });
  };

  if (route.kind === "note") {
    return <NoteEditorPage id={route.id} onBack={toLibrary} />;
  }

  if (route.kind === "settings") {
    return <SettingsPage onBack={toLibrary} />;
  }

  return (
    <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-5 p-4">
      <header className="flex items-center justify-between gap-1">
        {/* The wordmark is the one place the brand gradient is spent in full;
            everywhere else the theme shows up only as the accent. Build details
            used to sit under it; they live at the foot of Settings now, where
            nobody has to read past them to reach their notes. */}
        <h1 className="min-w-0 flex-1 bg-[image:var(--app-brand)] bg-clip-text text-2xl font-semibold tracking-tight text-transparent">
          xima.keeps
        </h1>
        {/* Language sits beside Settings rather than inside it: someone who has
            opened the app in a language they cannot read needs the way out to be
            visible on the first screen, not behind a word they cannot parse. */}
        <button
          type="button"
          aria-label={t("language.title")}
          aria-expanded={languageOpen}
          onClick={() => {
            setLanguageOpen(true);
          }}
          className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <Languages className="size-5" />
        </button>
        <button
          type="button"
          aria-label={t("settings.title")}
          onClick={() => {
            setRoute({ kind: "settings" });
          }}
          className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <Settings className="size-5" />
        </button>
      </header>

      {languageOpen && (
        <LanguagePicker
          onClose={() => {
            setLanguageOpen(false);
          }}
        />
      )}

      <LibraryPage
        onOpen={(id) => {
          setRoute({ kind: "note", id });
        }}
      />
    </main>
  );
}

export function App(): React.JSX.Element {
  return (
    <I18nProvider>
      <QueryClientProvider client={queryClient}>
        <Shell />
      </QueryClientProvider>
    </I18nProvider>
  );
}
