/**
 * The Android back gesture.
 *
 * The activity hands back to the WebView first and only finishes the app once
 * history has nothing left to pop, so anything that should close on back — a
 * screen, a sheet, a popover — has to own a history entry while it is open.
 *
 * Layers register here instead of each adding its own `popstate` listener: with
 * two open at once (a note plus its colour picker) every listener would answer
 * the same pop and the app would unwind both levels on one swipe.
 */

/** Open layers, innermost last. The top one answers the next back gesture. */
const handlers: (() => void)[] = [];

/** History entries this module pushed. Reconciled against `handlers`. */
let owned = 0;

/** Pops we asked for ourselves, which must not read as a back gesture. */
let selfPops = 0;

let pending: number | null = null;
let listening = false;

function onPopState(): void {
  if (selfPops > 0) {
    selfPops -= 1;
    return;
  }
  owned = Math.max(0, owned - 1);
  handlers.pop()?.();
}

/**
 * Brings the history depth back in line with the number of open layers.
 *
 * Deferred rather than applied on the spot because `history.back()` is
 * asynchronous and StrictMode mounts every effect twice: reconciling in a later
 * task lets an open-close-open pair inside one tick settle to a single entry
 * instead of leaving a dead back press behind in development builds.
 */
function reconcile(): void {
  pending = null;
  while (owned < handlers.length) {
    owned += 1;
    window.history.pushState({ backGuard: owned }, "");
  }
  while (owned > handlers.length) {
    owned -= 1;
    selfPops += 1;
    window.history.back();
  }
}

function schedule(): void {
  if (pending === null) {
    pending = window.setTimeout(reconcile, 0);
  }
}

export function pushBackGuard(handler: () => void): void {
  if (!listening) {
    listening = true;
    window.addEventListener("popstate", onPopState);
  }
  handlers.push(handler);
  schedule();
}

export function removeBackGuard(handler: () => void): void {
  const index = handlers.lastIndexOf(handler);
  if (index === -1) {
    // Already taken off the stack by the gesture that ran it.
    return;
  }
  handlers.splice(index, 1);
  schedule();
}
