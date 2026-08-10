import { useCallback, useEffect, useRef, useState } from "react";

/**
 * How fast the meter catches up with the microphone, per frame.
 *
 * Android reports loudness about ten times a second, which is slow enough to
 * see as steps and fast enough that a CSS transition on each one rubber-bands
 * instead of smoothing. So the readings are treated as a target and the drawn
 * value eases towards it every frame: at 0.25 a change lands in roughly six
 * frames — a tenth of a second, which is the gap between readings.
 */
const CATCH_UP = 0.25;

/** Below this the two values are the same to the eye, and the loop can stop. */
const SETTLED = 0.005;

/**
 * A microphone level that moves smoothly at sixty frames a second.
 *
 * The readings themselves arrive in bursts and jump; drawn straight, the halo
 * strobes rather than breathes. Easing towards the latest reading in an
 * animation frame also keeps the work off React's render path — the value is
 * held in a ref and only pushed into state when it has visibly moved.
 *
 * `prefers-reduced-motion` is respected by the global stylesheet, which kills
 * CSS transitions; this loop is what still lets the meter mean something there,
 * because it animates a value rather than a transition.
 */
export function useLevelMeter(): {
  readonly level: number;
  readonly push: (level: number) => void;
} {
  const [level, setLevel] = useState(0);
  const target = useRef(0);
  const current = useRef(0);
  const frame = useRef<number | null>(null);

  const step = useCallback((): void => {
    const distance = target.current - current.current;
    if (Math.abs(distance) < SETTLED) {
      current.current = target.current;
      setLevel(target.current);
      frame.current = null;
      return;
    }
    current.current += distance * CATCH_UP;
    setLevel(current.current);
    frame.current = window.requestAnimationFrame(step);
  }, []);

  const push = useCallback(
    (next: number): void => {
      target.current = next;
      if (frame.current === null) {
        frame.current = window.requestAnimationFrame(step);
      }
    },
    [step],
  );

  useEffect(() => {
    return () => {
      if (frame.current !== null) {
        window.cancelAnimationFrame(frame.current);
      }
    };
  }, []);

  return { level, push };
}
