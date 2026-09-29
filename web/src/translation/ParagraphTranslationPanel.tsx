import type { ComponentChildren } from "preact";
import { Icon } from "../ui/Icon";
import { useFloatingPanel } from "../ui/useFloatingPanel";

export function ParagraphTranslationPanel({
  pending,
  onClose,
  children,
}: {
  pending: boolean;
  onClose: () => void;
  children: ComponentChildren;
}) {
  const floating = useFloatingPanel(false);
  return (
    <div
      ref={floating.element}
      style={floating.style}
      class="paragraph-result"
      role="region"
      aria-label="Paragraph translation"
      aria-busy={pending}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <header>
        <div
          class="paragraph-result-handle"
          role="button"
          tabIndex={0}
          aria-label="Move paragraph translation with arrow keys or drag"
          {...floating.handle}
        >
          <strong>Перевод абзаца</strong>
        </div>
        <button
          class="icon-button"
          aria-label="Close paragraph translation"
          onClick={onClose}
        >
          <Icon name="close" />
        </button>
      </header>
      {children}
    </div>
  );
}
