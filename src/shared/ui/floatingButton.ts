/**
 * The row of buttons floating over the library.
 *
 * The classes live here rather than in each file because the three buttons sit
 * side by side: any drift in size, radius or vertical offset is visible at a
 * glance, and they are declared in two different components.
 *
 * `RIGHT` positions are the same 56px button plus an 8px gap, spelled out so the
 * row cannot be re-ordered into an uneven rhythm by accident.
 */

const SHAPE =
  "fixed bottom-6 z-30 flex size-14 items-center justify-center rounded-2xl shadow-lg";

/** Secondary buttons: the theme and the templates, quiet next to the accent. */
export const FLOATING_BUTTON_SECONDARY = `${SHAPE} bg-surface-raised border-border-subtle text-content-muted border`;

/** The one primary action on the screen. */
export const FLOATING_BUTTON_PRIMARY = `${SHAPE} bg-accent text-accent-content`;

export const FLOATING_RIGHT_PRIMARY = "right-5";
export const FLOATING_RIGHT_SECOND = "right-21";
export const FLOATING_RIGHT_THIRD = "right-37";
