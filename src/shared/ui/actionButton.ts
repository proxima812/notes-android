/**
 * The row of actions at the top of the library.
 *
 * The classes live here rather than in each file because the buttons sit side by
 * side and are declared in two different components: any drift in size, radius or
 * border is visible at a glance. They used to float over the bottom of the list,
 * which put them on top of the notes they were meant to serve; at the top they
 * share a line with the tabs and cover nothing.
 */

const SHAPE = "flex size-11 shrink-0 items-center justify-center rounded-2xl transition-colors";

/** Quiet actions: templates and the theme, next to the accent. */
export const ACTION_BUTTON_SECONDARY = `${SHAPE} bg-surface-raised border-border-subtle text-content-muted border`;

/** The one primary action on the screen. */
export const ACTION_BUTTON_PRIMARY = `${SHAPE} bg-accent text-accent-content`;
