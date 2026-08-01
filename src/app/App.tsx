import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

/** Shape returned by the Rust `app_info` command. Validated before use. */
interface AppInfo {
  readonly name: string;
  readonly version: string;
  readonly dataDir: string;
  readonly platform: string;
}

function isAppInfo(value: unknown): value is AppInfo {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const record: Record<string, unknown> = value as Record<string, unknown>;
  return (
    typeof record["name"] === "string" &&
    typeof record["version"] === "string" &&
    typeof record["dataDir"] === "string" &&
    typeof record["platform"] === "string"
  );
}

async function fetchAppInfo(): Promise<AppInfo> {
  const raw: unknown = await invoke("app_info");
  if (!isAppInfo(raw)) {
    throw new Error("Ядро вернуло неизвестный формат сведений о приложении");
  }
  return raw;
}

function Diagnostics(): React.JSX.Element {
  const { data, error, isPending } = useQuery({
    queryKey: ["app-info"],
    queryFn: fetchAppInfo,
    retry: false,
  });

  if (isPending) {
    return <p className="text-content-muted">Подключение к ядру…</p>;
  }

  if (error !== null) {
    return (
      <p className="text-danger">
        Ядро недоступно: {error instanceof Error ? error.message : "неизвестная ошибка"}
      </p>
    );
  }

  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
      <dt className="text-content-muted">Версия</dt>
      <dd className="font-mono">{data.version}</dd>
      <dt className="text-content-muted">Платформа</dt>
      <dd className="font-mono">{data.platform}</dd>
      <dt className="text-content-muted">Данные</dt>
      <dd className="font-mono break-all">{data.dataDir}</dd>
    </dl>
  );
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Everything is local; refetching on focus buys nothing and costs battery.
      refetchOnWindowFocus: false,
      staleTime: 30_000,
    },
  },
});

type Theme = "light" | "dark" | "oled";

export function App(): React.JSX.Element {
  const [theme, setTheme] = useState<Theme>("dark");
  document.documentElement.dataset["theme"] = theme;

  return (
    <QueryClientProvider client={queryClient}>
      <main className="mx-auto flex min-h-dvh max-w-md flex-col gap-6 p-5">
        <header>
          <h1 className="text-2xl font-semibold tracking-tight">Органайзер</h1>
          <p className="text-content-muted text-sm">Локальные заметки, задачи и напоминания</p>
        </header>

        <section className="bg-surface-raised border-border-subtle rounded-2xl border p-4">
          <h2 className="mb-3 text-sm font-medium">Состояние ядра</h2>
          <Diagnostics />
        </section>

        <section className="flex gap-2">
          {(["light", "dark", "oled"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => {
                setTheme(value);
              }}
              className={`min-h-11 flex-1 rounded-xl border px-4 text-sm transition-colors ${
                theme === value
                  ? "bg-accent text-accent-content border-transparent"
                  : "bg-surface-raised border-border-subtle"
              }`}
            >
              {value}
            </button>
          ))}
        </section>
      </main>
    </QueryClientProvider>
  );
}
