import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { appInfo } from "@/features/notes/api";
import { LibraryPage } from "@/pages/LibraryPage";
import { NoteEditorPage } from "@/pages/NoteEditorPage";
import { describeError } from "@/shared/api/errors";
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

function Diagnostics(): React.JSX.Element {
  const { data, error, isPending } = useQuery({
    queryKey: ["app-info"],
    queryFn: appInfo,
  });

  if (isPending) {
    return <p className="text-content-muted text-sm">Подключение к ядру…</p>;
  }
  if (error !== null) {
    return <p className="text-danger text-sm">{describeError(error)}</p>;
  }

  return (
    <p className="text-content-muted text-sm">
      Ядро {data.version} · схема v{data.schemaVersion} · {data.platform} ·{" "}
      {data.noteCount} заметок
    </p>
  );
}

/**
 * Two screens, so the route is a piece of state rather than a router: adding a
 * dependency to model "list or one note" would be more machinery than the app
 * has decisions to make.
 */
function Shell(): React.JSX.Element {
  const [openNote, setOpenNote] = useState<NoteId | null>(null);

  if (openNote !== null) {
    return (
      <NoteEditorPage
        id={openNote}
        onBack={() => {
          setOpenNote(null);
        }}
      />
    );
  }

  return (
    <main className="mx-auto flex min-h-dvh max-w-md flex-col gap-5 p-4">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight">Заметки</h1>
        <Diagnostics />
      </header>
      <LibraryPage onOpen={setOpenNote} />
    </main>
  );
}

export function App(): React.JSX.Element {
  document.documentElement.dataset["theme"] = "dark";

  return (
    <QueryClientProvider client={queryClient}>
      <Shell />
    </QueryClientProvider>
  );
}
