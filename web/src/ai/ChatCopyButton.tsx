import { useEffect, useRef, useState } from "preact/hooks";
import { Icon } from "../ui/Icon";

export function ChatCopyButton({ text, label, html }: { text: string; label: string; html?: () => string }) {
  const [state, setState] = useState<"idle" | "pending" | "copied" | "failed">("idle");
  const locked = useRef(false);
  const revision = useRef(0);
  useEffect(() => { revision.current++; locked.current = false; setState("idle"); }, [text]);
  const title = state === "copied" ? "Copied" : state === "pending" ? "Copying…" : state === "failed" ? "Copy failed — select the text to copy" : label;
  return <button class="icon-button ai-copy" type="button" aria-label={label} title={title}
    aria-busy={state === "pending"} disabled={!text || state === "pending"} data-copy-state={state}
    onClick={async () => {
      if (locked.current || !text) return;
      locked.current = true; setState("pending");
      const token = revision.current;
      try {
        if (html && typeof ClipboardItem !== "undefined" && navigator.clipboard.write) {
          await navigator.clipboard.write([new ClipboardItem({"text/plain":new Blob([text],{type:"text/plain"}),"text/html":new Blob([html()],{type:"text/html"})})]);
        } else await navigator.clipboard.writeText(text);
        if (token === revision.current) setState("copied");
      }
      catch { if (token === revision.current) setState("failed"); }
      finally { if (token === revision.current) locked.current = false; }
    }}>
    {state === "pending" ? <span class="spinner"/> : <Icon name={state === "copied" ? "check" : state === "failed" ? "close" : "copy"} size={16}/>}
    <span class="sr-only" aria-live="polite">{state !== "idle" ? title : ""}</span>
  </button>;
}
