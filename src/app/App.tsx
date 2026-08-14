import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AlarmClock, Languages, Search, Settings } from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";

import { listAppIcons } from "@/features/appearance/api";
import { reconcileReminderZone, topUpReminders } from "@/features/reminders/api";
import { useReminderLaunchTarget } from "@/features/reminders/useReminderLaunchTarget";
import { LanguagePicker } from "@/features/settings/ui/LanguagePicker";
import { SettingsPage } from "@/features/settings/ui/SettingsPage";
import { useDictationLaunch } from "@/features/quick-notes/useDictationLaunch";
import { LibraryPage } from "@/pages/LibraryPage";
import { RemindersPage } from "@/pages/RemindersPage";
import { SearchPage } from "@/pages/SearchPage";

import { I18nProvider, useT } from "@/shared/i18n";
import { useAppName } from "@/shared/lib/useAppName";
import type { NoteId } from "@/shared/types/ids";

/**
 * The editor is loaded when a note is opened, not before.
 *
 * It carries the whole rich-text stack, which is most of the JavaScript in the
 * app and none of what the library screen needs. Paying for it on every cold
 * start bought nothing: the first thing anyone sees is a list.
 */
const NoteEditorPage = lazy(async () => {
  const module = await import("@/pages/NoteEditorPage");
  return { default: module.NoteEditorPage };
});

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

/**
 * Shown for the moment the editor takes to arrive.
 *
 * Deliberately the same wording and placement the editor itself uses while it
 * reads the note, so opening a note does not flash two different waiting
 * states one after the other.
 */
function EditorLoading(): React.JSX.Element {
  const t = useT();
  return <p className="text-content-muted p-4 text-sm">{t("common.loading")}</p>;
}

type Route =
  | { readonly kind: "library" }
  | { readonly kind: "note"; readonly id: NoteId }
  | { readonly kind: "search" }
  | { readonly kind: "reminders" }
  | { readonly kind: "settings" };

/**
 * Five screens, so the route is a piece of state rather than a router: adding a
 * dependency to model "list, one note, search, reminders, or settings" would be
 * more machinery than the app has decisions to make.
 */
function Shell(): React.JSX.Element {
  const [route, setRoute] = useState<Route>({ kind: "library" });
  const [languageOpen, setLanguageOpen] = useState(false);
  // The launcher's «Продиктовать» opens the app straight into the microphone.
  const dictateOnOpen = useDictationLaunch();
  const [appName] = useAppName();
  const t = useT();

  // A reminder means a time on a clock, so crossing a border has to move the
  // instant rather than the time. Nothing announces that the device changed
  // zone, so the question is asked once per start and is usually answered
  // "nothing moved".
  useEffect(() => {
    void reconcileReminderZone()
      // Topping up after the move, so a repeat is armed further ahead at the
      // instants the new zone gives it rather than the old one.
      .then(topUpReminders)
      .catch(() => {
        // The app must still open. A reminder left in yesterday's zone is worth
        // less than a start screen that refuses to appear.
      });

    // Asking which icon is showing also puts right an update that left two of
    // them enabled — component states the app set survive an install, and the
    // manifest enables its own default on top of them.
    void listAppIcons().catch(() => {
      // Off-device there is no launcher to ask, and that is not an error worth
      // showing on the first screen.
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
    return (
      <Suspense fallback={<EditorLoading />}>
        <NoteEditorPage id={route.id} onBack={toLibrary} />
      </Suspense>
    );
  }

  if (route.kind === "search") {
    return (
      <SearchPage
        onBack={toLibrary}
        onOpen={(id) => {
          setRoute({ kind: "note", id });
        }}
      />
    );
  }

  if (route.kind === "reminders") {
    return (
      <RemindersPage
        onBack={toLibrary}
        onOpen={(id) => {
          setRoute({ kind: "note", id });
        }}
      />
    );
  }

  if (route.kind === "settings") {
    return <SettingsPage onBack={toLibrary} />;
  }

  return (
    <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-5 p-4">
      <header className="flex items-center justify-between gap-1">
        {/* The name spelled out rather than the mark: an icon says nothing to
            someone who renamed the app, and the name is theirs to change in
            Settings. Ten characters is the budget, which is what keeps this
            from crowding the three controls beside it. Build details used to
            sit under it; they live at the foot of Settings now, where nobody
            has to read past them to reach their notes. */}
        <h1 className="min-w-0 flex-1 truncate text-xl font-semibold tracking-tight">{appName}</h1>
        {/* Search is a screen of its own, reached from here rather than from a
            field pinned over the list: the library is what people came for, and
            search is what they ask for when it did not have what they wanted. */}
        <button
          type="button"
          aria-label={t("library.search")}
          onClick={() => {
            setRoute({ kind: "search" });
          }}
          className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <Search className="size-5" />
        </button>
        {/* Language sits beside Settings rather than inside it: someone who has
            opened the app in a language they cannot read needs the way out to be
            visible on the first screen, not behind a word they cannot parse. */}
        {/* What is going to happen, as opposed to what was written. Beside
            search because both are ways of asking the library a question it
            cannot answer by being scrolled. */}
        <button
          type="button"
          aria-label={t("reminders.title")}
          onClick={() => {
            setRoute({ kind: "reminders" });
          }}
          className="text-content-muted flex size-11 shrink-0 items-center justify-center rounded-full"
        >
          <AlarmClock className="size-5" />
        </button>
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
        dictateOnOpen={dictateOnOpen}
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
