import { usePanelDock } from "./PanelDock";
import type { ComponentChildren, JSX } from "preact";
import type { useFloatingPanel } from "./useFloatingPanel";

/** Fixed overlays own their geometry. Response content must only scroll inside
 * their reserved content region; it cannot move the article or action targets. */
export function FloatingPanel({
  position,
  label,
  onClose,
  children,
  class: className,
  onPointerDown,
}: {
  position: ReturnType<typeof useFloatingPanel>;
  label: string;
  onClose: () => void;
  children: ComponentChildren;
  class: string;
  onPointerDown?: JSX.PointerEventHandler<HTMLDivElement>;
}) {
  const dock = usePanelDock();
  return (
    <div
      ref={position.element}
      class={className}
      style={dock ? undefined : position.style}
      role="dialog"
      aria-modal="false"
      aria-label={label}
      onPointerDown={onPointerDown}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      {children}
    </div>
  );
}
