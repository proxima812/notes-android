import { beforeEach, describe, expect, it } from "vitest";

import { pushBackGuard, removeBackGuard } from "./backGuard";

/**
 * Both `history.back()` and the module's own reconciliation are asynchronous, so
 * every assertion has to wait for the queue to drain rather than for one tick.
 */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
  }
}

/** What the Android gesture ends up doing to the WebView. */
async function pressBack(): Promise<void> {
  window.history.back();
  await settle();
}

describe("backGuard", () => {
  beforeEach(async () => {
    // Each test starts from a history with nothing of ours left in it.
    await settle();
  });

  it("runs the innermost guard and leaves the outer one armed", async () => {
    const calls: string[] = [];
    const screen = (): void => {
      calls.push("screen");
    };
    const panel = (): void => {
      calls.push("panel");
    };

    pushBackGuard(screen);
    await settle();
    pushBackGuard(panel);
    await settle();

    await pressBack();
    expect(calls).toEqual(["panel"]);

    await pressBack();
    expect(calls).toEqual(["panel", "screen"]);
  });

  it("does not spend a back gesture on a layer that was closed by its own button", async () => {
    const calls: string[] = [];
    const screen = (): void => {
      calls.push("screen");
    };
    const panel = (): void => {
      calls.push("panel");
    };

    pushBackGuard(screen);
    await settle();
    pushBackGuard(panel);
    await settle();

    removeBackGuard(panel);
    await settle();

    await pressBack();
    expect(calls).toEqual(["screen"]);
  });

  it("keeps one entry when a guard is re-registered within the same tick", async () => {
    // What StrictMode does on mount: effect, cleanup, effect.
    const calls: string[] = [];
    const screen = (): void => {
      calls.push("screen");
    };

    pushBackGuard(screen);
    removeBackGuard(screen);
    pushBackGuard(screen);
    await settle();

    await pressBack();
    expect(calls).toEqual(["screen"]);
  });
});
