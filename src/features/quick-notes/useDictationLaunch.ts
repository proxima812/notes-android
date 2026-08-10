import { useEffect, useState } from "react";

import { takeDictationRequest } from "./speech";

/**
 * True once the launcher's «Продиктовать» shortcut has asked for the
 * microphone.
 *
 * The shortcut arrives as an intent on the activity, not as an event in the
 * WebView, so — exactly like a tapped reminder — the plugin holds it until it
 * is asked. Asking happens on mount, which covers a cold start, and whenever
 * the document becomes visible, which covers a shortcut tapped while the app
 * was already running in the background.
 *
 * The flag is one-way. Turning it back off is the sheet's job when it closes;
 * the plugin has already forgotten the request by then, so a return from the
 * background does not reopen the microphone by itself.
 */
export function useDictationLaunch(): boolean {
  const [requested, setRequested] = useState(false);

  useEffect(() => {
    let stopped = false;

    const collect = (): void => {
      takeDictationRequest()
        .then((asked) => {
          if (asked && !stopped) {
            setRequested(true);
          }
        })
        .catch(() => {
          // Off-device, or a plugin that cannot be reached. The app still has
          // to open, and the microphone button is right there on the screen.
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

  return requested;
}
