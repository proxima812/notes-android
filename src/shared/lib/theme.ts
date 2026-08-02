import type { StringKey } from "@/shared/i18n";

/**
 * App themes.
 *
 * Only the identifier is persisted; the palette lives in `global.css` under
 * `[data-theme="..."]`. That split is the same one the note gradients use: a
 * repaint never touches stored state, and an identifier this build no longer
 * knows falls back to the default instead of rendering an unstyled screen.
 */

export interface AppTheme {
  readonly id: AppThemeId;
  /** Dictionary key; the label itself lives in the locale files. */
  readonly labelKey: StringKey;
  /** Picker preview. Mirrors `--app-brand` of the matching selector. */
  readonly swatch: string;
}

export const APP_THEME_IDS = [
  "mint",
  "indigo",
  "amethyst",
  "amber",
  "obsidian",
  "porcelain",
] as const;

export type AppThemeId = (typeof APP_THEME_IDS)[number];

/** The launcher icon's mint, so the app opens in the colour it was tapped in. */
export const DEFAULT_THEME_ID: AppThemeId = "mint";

export const APP_THEMES: readonly AppTheme[] = [
  {
    id: "mint",
    labelKey: "theme.mint",
    swatch: "linear-gradient(135deg, oklch(92% 0.121 168) 0%, oklch(81% 0.191 158) 100%)",
  },
  {
    id: "indigo",
    labelKey: "theme.indigo",
    swatch: "linear-gradient(135deg, oklch(74% 0.16 248) 0%, oklch(62% 0.19 285) 100%)",
  },
  {
    id: "amethyst",
    labelKey: "theme.amethyst",
    swatch: "linear-gradient(135deg, oklch(75% 0.17 308) 0%, oklch(64% 0.17 272) 100%)",
  },
  {
    id: "amber",
    labelKey: "theme.amber",
    swatch: "linear-gradient(135deg, oklch(88% 0.14 98) 0%, oklch(76% 0.16 58) 100%)",
  },
  {
    id: "obsidian",
    labelKey: "theme.obsidian",
    swatch: "linear-gradient(135deg, oklch(45% 0 90) 0%, oklch(0% 0 0) 100%)",
  },
  {
    id: "porcelain",
    labelKey: "theme.porcelain",
    swatch: "linear-gradient(135deg, oklch(100% 0 90) 0%, oklch(88% 0.004 286) 100%)",
  },
];

const STORAGE_KEY = "xkeeps.theme";

export function isThemeId(value: string | null): value is AppThemeId {
  return value !== null && (APP_THEME_IDS as readonly string[]).includes(value);
}

/**
 * Reading storage is wrapped because a WebView with site data disabled throws
 * here rather than returning null, and losing the saved theme must never take
 * the whole app down with it.
 */
export function loadTheme(): AppThemeId {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return isThemeId(stored) ? stored : DEFAULT_THEME_ID;
  } catch {
    return DEFAULT_THEME_ID;
  }
}

export function saveTheme(id: AppThemeId): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // A theme that survives only this session still beats a crash.
  }
}

export function applyTheme(id: AppThemeId): void {
  document.documentElement.dataset["theme"] = id;
}
