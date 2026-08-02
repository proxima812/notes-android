import {
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

/** How far the card must travel before letting go runs the action. */
const COMMIT_PX = 96;

/** Movement before the gesture is claimed as a swipe rather than a scroll. */
const SLOP_PX = 10;

/** Past the commit point the card keeps moving, but grudgingly. */
const RESISTANCE = 0.35;

interface CardSwipeOptions {
  readonly onSwipeLeft: () => void;
  readonly onSwipeRight: () => void;
  readonly disabled: boolean;
}

interface CardSwipe {
  /** Current horizontal offset of the card, in pixels. */
  readonly dx: number;
  /** 0…1 — how close the gesture is to committing. Drives the icon. */
  readonly progress: number;
  /** Letting go now would run the action. */
  readonly armed: boolean;
  /** A finger is on the card and moving it; suppresses the settle animation. */
  readonly dragging: boolean;
  readonly handlers: {
    readonly onPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
    readonly onPointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
    readonly onPointerUp: (event: ReactPointerEvent<HTMLElement>) => void;
    readonly onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
    readonly onClickCapture: (event: ReactMouseEvent<HTMLElement>) => void;
  };
}

/**
 * Horizontal swipe over a list row.
 *
 * The gesture is claimed only once the finger has moved further sideways than
 * down: a list is scrolled far more often than a row is swiped, so an ambiguous
 * drag has to stay with the scroller. Until then the pointer is not captured and
 * the card does not move at all, which is also what keeps a tap a tap.
 */
export function useCardSwipe({
  onSwipeLeft,
  onSwipeRight,
  disabled,
}: CardSwipeOptions): CardSwipe {
  const [dx, setDx] = useState(0);
  const [dragging, setDragging] = useState(false);

  const start = useRef<{ x: number; y: number; id: number } | null>(null);
  const claimed = useRef(false);
  /** Set for the tail of a swipe, so releasing does not also open the note. */
  const swiped = useRef(false);

  const reset = (): void => {
    start.current = null;
    claimed.current = false;
    setDragging(false);
    setDx(0);
  };

  return {
    dx,
    progress: Math.min(Math.abs(dx) / COMMIT_PX, 1),
    armed: Math.abs(dx) >= COMMIT_PX,
    dragging,
    handlers: {
      onPointerDown: (event) => {
        if (disabled || (event.pointerType === "mouse" && event.button !== 0)) {
          return;
        }
        start.current = { x: event.clientX, y: event.clientY, id: event.pointerId };
        claimed.current = false;
        swiped.current = false;
      },

      onPointerMove: (event) => {
        const origin = start.current;
        if (origin === null || event.pointerId !== origin.id) {
          return;
        }
        const moveX = event.clientX - origin.x;
        const moveY = event.clientY - origin.y;

        if (!claimed.current) {
          if (Math.abs(moveY) > SLOP_PX && Math.abs(moveY) >= Math.abs(moveX)) {
            // The list is being scrolled; hand the gesture back.
            start.current = null;
            return;
          }
          if (Math.abs(moveX) <= SLOP_PX) {
            return;
          }
          claimed.current = true;
          swiped.current = true;
          setDragging(true);
          event.currentTarget.setPointerCapture(origin.id);
        }

        // The slop is subtracted so the card does not jump on the frame the
        // gesture is claimed.
        const travel = moveX - Math.sign(moveX) * SLOP_PX;
        const over = Math.abs(travel) - COMMIT_PX;
        setDx(
          over <= 0
            ? travel
            : Math.sign(travel) * (COMMIT_PX + over * RESISTANCE),
        );
      },

      onPointerUp: (event) => {
        const origin = start.current;
        if (origin === null || event.pointerId !== origin.id) {
          return;
        }
        const committed = Math.abs(dx) >= COMMIT_PX;
        const direction = dx;
        reset();
        if (committed) {
          if (direction < 0) {
            onSwipeLeft();
          } else {
            onSwipeRight();
          }
        }
      },

      onPointerCancel: reset,

      // A swipe that ends over the open button would otherwise also be a click.
      onClickCapture: (event) => {
        if (swiped.current) {
          event.preventDefault();
          event.stopPropagation();
          swiped.current = false;
        }
      },
    },
  };
}
