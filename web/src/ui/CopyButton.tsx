import { useEffect, useRef, useState } from "preact/hooks";
import { Icon } from "../ui/Icon";

export function CopyButton({
  text,
  label,
  html,
  className = "icon-button ai-copy",
  disabled = false,
  hint,
  visibleLabel,
}: {
  text: string | (() => string);
  className?: string;
  disabled?: boolean;
  hint?: string;
  visibleLabel?: string;
  label: string;
  html?: () => string;
}) {
  const [state, setState] = useState<"idle" | "pending" | "copied" | "failed">(
    "idle",
  );
  const locked = useRef(false);
  const revision = useRef(0);
  useEffect(() => {
    revision.current++;
    locked.current = false;
    setState("idle");
  }, [text]);
  const title =
    state === "copied"
      ? "Copied"
      : state === "pending"
        ? "Copying…"
        : state === "failed"
          ? "Copy failed — select the text to copy"
          : (hint ?? label);
  return (
    <button
      class={className}
      type="button"
      aria-label={label}
      title={title}
      aria-busy={state === "pending"}
      disabled={disabled || !text || state === "pending"}
      data-copy-state={state}
      onClick={async () => {
        if (locked.current || disabled || !text) return;
        locked.current = true;
        setState("pending");
        const token = revision.current;
        try {
          const value = typeof text === "function" ? text() : text;
          if (!value) throw new Error("No article content to copy");
          if (
            html &&
            typeof ClipboardItem !== "undefined" &&
            navigator.clipboard.write
          ) {
            await navigator.clipboard.write([
              new ClipboardItem({
                "text/plain": new Blob([value], { type: "text/plain" }),
                "text/html": new Blob([html()], { type: "text/html" }),
              }),
            ]);
          } else await navigator.clipboard.writeText(value);
          if (token === revision.current) setState("copied");
        } catch {
          if (token === revision.current) setState("failed");
        } finally {
          if (token === revision.current) locked.current = false;
        }
      }}
    >
      {state === "pending" ? (
        <span class="spinner" />
      ) : (
        <Icon
          name={
            state === "copied" ? "check" : state === "failed" ? "close" : "copy"
          }
          size={16}
        />
      )}
      {visibleLabel && (
        <span class="reader-toolbar-button__label">{visibleLabel}</span>
      )}
      <span class="sr-only" aria-live="polite">
        {state !== "idle" ? title : ""}
      </span>
    </button>
  );
}
