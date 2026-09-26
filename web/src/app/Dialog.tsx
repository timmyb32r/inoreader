import type { ComponentChildren } from "preact";
import { useId, useLayoutEffect, useRef } from "preact/hooks";
import { Icon } from "../ui/Icon";

export function Dialog({ title, description, children, onClose, width = "520px" }: { title: string; description?: string; children: ComponentChildren; onClose: () => void; width?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const titleId = useId();
  const descriptionId = useId();
  useLayoutEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = ref.current;
    (dialog?.querySelector<HTMLElement>("[autofocus]") ?? dialog)?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (dialog?.closest("[hidden]") || event.defaultPrevented) return;
      const dialogs = [...document.querySelectorAll('[role="dialog"]')].filter(item => !item.closest("[hidden]"));
      if (dialogs.at(-1) !== dialog) return;
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); closeRef.current(); return; }
      if (event.key !== "Tab" || !dialog) return;
      const focusable = [...dialog.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])')];
      if (focusable.length === 0) { event.preventDefault(); dialog.focus(); return; }
      const first = focusable[0], last = focusable[focusable.length - 1];
      if (document.activeElement === dialog) { event.preventDefault(); (event.shiftKey ? last : first).focus(); }
      else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener("keydown", keyboard);
    return () => { document.removeEventListener("keydown", keyboard); previouslyFocused?.focus(); };
  }, []);
  return <div class="modal-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined} tabIndex={-1} ref={ref} style={{ maxWidth: width }}>
      <header class="modal__header"><div><p class="eyebrow">Reader workspace</p><h2 id={titleId}>{title}</h2>{description && <p id={descriptionId}>{description}</p>}</div><button class="icon-button" aria-label="Close dialog" onClick={onClose}><Icon name="close" /></button></header>
      {children}
    </div>
  </div>;
}
