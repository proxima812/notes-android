import { useEffect, useRef } from "react";

import type { NoteId } from "@/shared/types/ids";

import { takeReminderLaunchTarget } from "./api";

/**
 * Opens the note behind a tapped reminder notification.
 *
 * Android hands the tap to the activity, not to the WebView, so there is no
 * event to subscribe to here: the core holds the note until it is asked for it.
 * Asking happens on mount, which covers a cold start, and every time the
 * document becomes visible, which covers a tap that only brought an already
 * running app back to the front. Repeated asks are free — the core clears the
 * target as it answers.
 *
 * The callback is read through a ref so an inline arrow does not reattach the
 * listener on every render.
 */
export function useReminderLaunchTarget(onOpen: (id: NoteId) => void): void {
  const latest = useRef(onOpen);

  useEffect(() => {
    latest.current = onOpen;
  }, [onOpen]);

  useEffect(() => {
    let stopped = false;

    const collect = (): void => {
      void takeReminderLaunchTarget()
        .then((id) => {
          if (id !== null && !stopped) {
            latest.current(id);
          }
        })
        .catch(() => {
          // Nothing the user can act on, and the app must still open: a note
          // that fails to surface is worse as a blocked start than as a missed
          // navigation.
        });
    };

    collect();
    const onVisibility = (): void => {
      if (document.visibilityState === "visible") {
        collect();
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      stopped = true;
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);
}
