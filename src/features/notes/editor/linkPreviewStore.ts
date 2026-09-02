/**
 * The editor's memory of what the links in it point at.
 *
 * A module-level store rather than React state: the thing that needs an icon is
 * a ProseMirror mark being rendered, which is not a component and cannot hold a
 * hook. The core is the real cache — this only keeps the answers already given
 * during this run of the app, so scrolling through a note does not re-ask the
 * bridge for every link on every redraw.
 */

import { fetchLinkPreview, type LinkPreview } from "@/features/links/api";

/**
 * `undefined` — never asked. `null` — asked, and the address is not one the app
 * fetches. Otherwise the answer, which may itself be empty.
 */
const answers = new Map<string, LinkPreview | null>();
const asking = new Set<string>();
const listeners = new Set<() => void>();

/** How many addresses are read at once, so a pasted wall of links queues. */
const PARALLEL = 3;
const queue: string[] = [];
/**
 * The addresses currently in the air.
 *
 * A set rather than a count: a count has to be decremented exactly once per
 * request, and anything that drops the queue on the floor — forgetting
 * everything, say — leaves it describing work that no longer exists, after
 * which nothing new is ever started. A set can only be wrong about which
 * addresses are in flight, and clearing it is a complete answer.
 */
const inFlight = new Set<string>();

/**
 * Bumped by [`forgetKnownPreviews`], and checked when an answer lands.
 *
 * Without it, forgetting is only as final as the network is quiet: a request
 * already in the air would land afterwards and write its answer back into the
 * map that was just emptied. Anything asked for before the sweep is dropped
 * when it arrives.
 */
let generation = 0;

function announce(): void {
  for (const listener of listeners) {
    listener();
  }
}

/** Runs one queued address, then the next, keeping at most `PARALLEL` in the air. */
function pump(): void {
  while (inFlight.size < PARALLEL) {
    const href = queue.shift();
    if (href === undefined) {
      return;
    }
    inFlight.add(href);
    const asked = generation;
    void fetchLinkPreview(href)
      .then((preview) => {
        if (asked !== generation) {
          return;
        }
        answers.set(href, preview);
        // Also under the address the core normalised it to, so a second link
        // that only differs by its fragment is answered without a second trip.
        if (preview !== null) {
          answers.set(preview.url, preview);
        }
      })
      .catch(() => {
        if (asked !== generation) {
          return;
        }
        // A bridge failure is not worth a message on the screen: the link still
        // works, it just has no icon. Remembered as «nothing» so it is not
        // retried on every keystroke.
        answers.set(href, null);
      })
      .finally(() => {
        inFlight.delete(href);
        asking.delete(href);
        announce();
        pump();
      });
  }
}

/** What is known about an address right now, without asking. */
export function knownPreview(href: string): LinkPreview | null | undefined {
  return answers.get(href);
}

/** Asks about an address, unless it is already known or already being asked. */
export function askAbout(href: string): void {
  if (href === "" || answers.has(href) || asking.has(href)) {
    return;
  }
  asking.add(href);
  queue.push(href);
  pump();
}

/** Called whenever an answer arrives. Returns the unsubscribe. */
export function onPreviewsChanged(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Drops what this run has learned, including anything still in the air. */
export function forgetKnownPreviews(): void {
  generation += 1;
  answers.clear();
  asking.clear();
  inFlight.clear();
  queue.length = 0;
  announce();
}
