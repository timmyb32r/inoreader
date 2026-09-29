import type { JSX } from "preact";
import { useLayoutEffect, useRef, useState } from "preact/hooks";

/** Position affects only the fixed overlay. Dragging starts exclusively on its
 * handle; resizing clamps its bounds so close/minimize never leave the viewport. */
export function useFloatingPanel(collapsed: boolean, enabled = true) {
  const element = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{
    left: number;
    top: number;
  } | null>(null);
  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  const clamp = (left: number, top: number) => {
    const bounds = element.current?.getBoundingClientRect();
    return {
      left: Math.max(
        8,
        Math.min(left, window.innerWidth - (bounds?.width ?? 440) - 8),
      ),
      top: Math.max(
        8,
        Math.min(top, window.innerHeight - (bounds?.height ?? 56) - 8),
      ),
    };
  };
  useLayoutEffect(() => {
    if (!enabled) return;
    const keepVisible = () => {
      const bounds = element.current?.getBoundingClientRect();
      if (bounds) setPosition(clamp(bounds.left, bounds.top));
    };
    keepVisible();
    window.addEventListener("resize", keepVisible);
    window.visualViewport?.addEventListener("resize", keepVisible);
    return () => {
      window.removeEventListener("resize", keepVisible);
      window.visualViewport?.removeEventListener("resize", keepVisible);
    };
  }, [collapsed, enabled]);
  const onPointerDown: JSX.PointerEventHandler<HTMLDivElement> = (event) => {
    if (event.button !== 0) return;
    const bounds = element.current?.getBoundingClientRect();
    if (!bounds) return;
    event.preventDefault();
    event.currentTarget.focus();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      x: event.clientX,
      y: event.clientY,
      left: bounds.left,
      top: bounds.top,
    };
  };
  const onPointerMove: JSX.PointerEventHandler<HTMLDivElement> = (event) => {
    if (drag.current)
      setPosition(
        clamp(
          drag.current.left + event.clientX - drag.current.x,
          drag.current.top + event.clientY - drag.current.y,
        ),
      );
  };
  const onPointerUp: JSX.PointerEventHandler<HTMLDivElement> = (event) => {
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
  };
  const onKeyDown: JSX.KeyboardEventHandler<HTMLDivElement> = (event) => {
    const delta: Record<string, [number, number]> = {
      ArrowLeft: [-20, 0],
      ArrowRight: [20, 0],
      ArrowUp: [0, -20],
      ArrowDown: [0, 20],
    };
    const movement = delta[event.key],
      bounds = element.current?.getBoundingClientRect();
    if (movement && bounds) {
      event.preventDefault();
      setPosition(clamp(bounds.left + movement[0], bounds.top + movement[1]));
    }
  };
  // CSS clamps in the same layout pass as a viewport change, before the browser
  // delivers its resize event. The event then updates the saved drag position.
  const style = position
    ? {
        left: `clamp(8px, ${position.left}px, calc(100vw - var(--chat-width) - 8px))`,
        top: `clamp(8px, ${position.top}px, calc(100dvh - var(--chat-height) - 8px))`,
        bottom: "auto",
        right: "auto",
      }
    : undefined;
  return {
    element,
    style,
    handle: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
      onKeyDown,
    },
  };
}
