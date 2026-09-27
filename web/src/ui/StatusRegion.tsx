import type { ComponentChildren } from "preact";
/** Mount unconditionally in a fixed-height panel row, including the empty state. */
export function StatusRegion({
  busy,
  children,
  title,
  class: className,
}: {
  busy?: boolean;
  title?: string;
  children: ComponentChildren;
  class?: string;
}) {
  return (
    <div
      class={className}
      title={title}
      role="status"
      aria-live="polite"
      aria-busy={busy}
    >
      {children}
    </div>
  );
}
