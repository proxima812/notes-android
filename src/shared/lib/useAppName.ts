import { useSyncExternalStore } from "react";

import { loadAppName, saveAppName, subscribeAppName } from "./appName";

/**
 * The current app name, and a setter that persists it.
 *
 * `useSyncExternalStore` rather than `useState` because the header on the
 * library screen stays mounted while Settings is open: it has to repaint when
 * the field there changes, and storage is the one place both agree on. The
 * snapshot is a plain string, so React's own equality check ends the render
 * when nothing actually changed.
 */
export function useAppName(): readonly [string, (value: string) => void] {
  const name = useSyncExternalStore(subscribeAppName, loadAppName, loadAppName);

  return [name, saveAppName];
}
