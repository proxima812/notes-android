/**
 * The name the app calls itself on its first screen.
 *
 * The header used to show the icon and nothing else; it now spells the name out,
 * and the name is something a person may replace. Ten characters is the whole
 * budget — the default, `xima.keeps`, is exactly that long, so anything that
 * still fits the line is a name the header can render without shrinking or
 * truncating it.
 *
 * It lives in `localStorage` beside the theme and the language rather than in
 * the SQLite settings table: it styles this install's UI and nothing the core
 * reads, migrates, or backs up.
 */

export const DEFAULT_APP_NAME = "xima.keeps";

/** Counted in code points, so an emoji costs one character rather than two. */
export const MAX_APP_NAME_LENGTH = 10;

const STORAGE_KEY = "xkeeps.name";

/**
 * Cuts a name to the budget without touching its spaces.
 *
 * This is what a field being typed into gets: trimming here would eat the space
 * in "my notes" the moment it was pressed, leaving "mynotes" behind. The cut is
 * on code points because slicing a string in the middle of a surrogate pair
 * leaves half a character, which renders as a replacement glyph in the header.
 */
export function limitAppName(value: string): string {
  return [...value].slice(0, MAX_APP_NAME_LENGTH).join("");
}

/** What a typed name becomes once stored: cut to the budget, then trimmed. */
export function normaliseAppName(value: string): string {
  return limitAppName(value).trim();
}

/**
 * The name the user chose, or `null` if they never chose one.
 *
 * Reading is wrapped because a WebView with site data disabled throws here, and
 * a lost name must not take the first screen down with it.
 */
export function loadStoredAppName(): string | null {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === null) {
      return null;
    }
    const name = normaliseAppName(stored);
    return name === "" ? null : name;
  } catch {
    return null;
  }
}

/** What to show right now: the chosen name, else the default. */
export function loadAppName(): string {
  return loadStoredAppName() ?? DEFAULT_APP_NAME;
}

/**
 * Stores a name, or clears it back to the default when the field is emptied.
 *
 * Clearing is the way back: a field with no undo, and no separate "reset"
 * button, still has to let someone who typed a name they dislike return to the
 * one the app shipped with.
 */
export function saveAppName(value: string): void {
  const name = normaliseAppName(value);
  try {
    if (name === "") {
      window.localStorage.removeItem(STORAGE_KEY);
    } else {
      window.localStorage.setItem(STORAGE_KEY, name);
    }
  } catch {
    // A name that survives only this session still beats a crash.
  }
  notify();
}

/**
 * The screens that show the name and the screen that edits it are different
 * screens, but the header outlives a trip to Settings, so it has to hear about
 * the change rather than re-read storage on its next mount.
 */
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

export function subscribeAppName(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
