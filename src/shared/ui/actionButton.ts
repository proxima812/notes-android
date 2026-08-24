/**
 * The row of actions at the top of the library, and the one that floats over it.
 *
 * The classes live here rather than in each file because the buttons sit side by
 * side and are declared in two different components: any drift in size or radius
 * is visible at a glance. Nothing here is outlined — depth is what separates a
 * control from the page it sits on, in every palette the app ships.
 */

const SHAPE = "flex size-11 shrink-0 items-center justify-center rounded-2xl transition-colors";

/** Quiet actions: templates and dictation. */
export const ACTION_BUTTON_SECONDARY = `${SHAPE} bg-surface-raised text-content-muted`;

/**
 * Creating a note: the button that floats over the library.
 *
 * Its own class rather than the primary one with sizes bolted on, because two
 * Tailwind utilities for the same property in one attribute do not reliably
 * override each other. It is deliberately larger than everything else on the
 * screen — it is the only thing the library screen is actually for — and it sits
 * in the bottom corner a thumb already rests in.
 */
export const ACTION_BUTTON_FLOATING =
  "bg-accent text-accent-content fixed right-12 bottom-12 z-30 flex size-16 items-center justify-center rounded-full shadow-lg transition-colors";
