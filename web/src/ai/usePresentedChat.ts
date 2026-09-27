import { useEffect, useRef, useState } from "preact/hooks";
import type { ArticleChat } from "../api/ai";

/** Defer remote text replacement during a press, selection or active scrolling.
 * The 180ms scroll-idle interval never delays feedback on a clicked control. */
export function usePresentedChat(chat: ArticleChat | null) {
  const [presented, setPresented] = useState(chat);
  const messages = useRef<HTMLDivElement>(null);
  const latest = useRef(chat);
  const pressed = useRef(false);
  const scrolling = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  latest.current = chat;
  const flush = () => {
    const selection = window.getSelection();
    const selecting = selection && !selection.isCollapsed &&
      (messages.current?.contains(selection.anchorNode) || messages.current?.contains(selection.focusNode));
    if (!pressed.current && !scrolling.current && !selecting) setPresented(latest.current);
  };
  useEffect(() => {
    // Opening another article must never display the previous article's text.
    if (chat?.id !== presented?.id) setPresented(chat);
    else flush();
  }, [chat]);
  useEffect(() => {
    const down = (event: PointerEvent) => {
      if (event.target instanceof Node && messages.current?.closest(".ai-chat")?.contains(event.target)) pressed.current = true;
    };
    const up = () => { pressed.current = false; flush(); };
    window.addEventListener("pointerdown", down);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    document.addEventListener("selectionchange", flush);
    return () => {
      clearTimeout(timer.current);
      window.removeEventListener("pointerdown", down);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      document.removeEventListener("selectionchange", flush);
    };
  }, []);
  const onScroll = () => {
    scrolling.current = true;
    clearTimeout(timer.current);
    timer.current = setTimeout(() => { scrolling.current = false; flush(); }, 180);
  };
  return { presented, messages, onScroll };
}
