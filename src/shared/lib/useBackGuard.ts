import { useEffect, useRef } from "react";

import { pushBackGuard, removeBackGuard } from "./backGuard";

/**
 * Closes a layer on the Android back gesture while `active`.
 *
 * The callback is read through a ref so that a handler rebuilt on every render —
 * which is what an inline arrow is — does not tear the guard down and push a
 * fresh history entry on each keystroke.
 */
export function useBackGuard(active: boolean, onBack: () => void): void {
  const latest = useRef(onBack);

  useEffect(() => {
    latest.current = onBack;
  }, [onBack]);

  useEffect(() => {
    if (!active) {
      return;
    }
    const handler = (): void => {
      latest.current();
    };
    pushBackGuard(handler);
    return () => {
      removeBackGuard(handler);
    };
  }, [active]);
}
