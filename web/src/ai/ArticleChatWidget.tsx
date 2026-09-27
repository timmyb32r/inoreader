import { useLayoutEffect, useRef } from "preact/hooks";
import { isChatActive, type AiProfile } from "../api/ai";
import { AutofillResistantTextarea } from "../ui/fields";
import { FloatingPanel } from "../ui/FloatingPanel";
import { Icon } from "../ui/Icon";
import { StatusRegion } from "../ui/StatusRegion";
import { useFloatingPanel } from "../ui/useFloatingPanel";
import { CopyButton } from "../ui/CopyButton";
import { ChatMarkdown } from "./ChatMarkdown";
import {
  chatProgress,
  hasVerifiedSummary,
  messagePlaceholder,
  visibleMessageContent,
} from "./chatPresentation";
import "./deepseek.css";
import type { ArticleChatController } from "./useArticleChat";
import { usePresentedChat } from "./usePresentedChat";

export function ArticleChatWidget({
  controller,
  profile,
}: {
  controller: ArticleChatController;
  profile: AiProfile | null;
}) {
  const { chat, target, collapsed, busy, error } = controller;
  const floating = useFloatingPanel(collapsed);
  const { presented, messages, onScroll } = usePresentedChat(chat);
  const thread = useRef<HTMLDivElement>(null);
  const prior = useRef({ id: "", count: 0, top: 0 });
  const active = isChatActive(chat);
  const canRetry =
    controller.retryable ||
    (!!chat && ["failed", "interrupted", "cancelled"].includes(chat.status));
  const summaryReady = hasVerifiedSummary(chat);
  const canSend =
    summaryReady &&
    !!profile?.enabled &&
    !busy &&
    !active &&
    !controller.unresolved &&
    !!controller.draft.trim();
  useLayoutEffect(() => {
    if (!collapsed)
      floating.element.current
        ?.querySelector<HTMLElement>("textarea:not(:disabled)")
        ?.focus();
  }, [collapsed, chat?.id]);
  useLayoutEffect(() => {
    const box = messages.current,
      body = thread.current;
    if (!box || !body) return;
    if (prior.current.id !== presented?.id) {
      box.scrollTop = 0;
      body.style.minHeight = "";
    } else if ((presented?.messages.length ?? 0) > prior.current.count)
      box.scrollTop = box.scrollHeight;
    else box.scrollTop = prior.current.top;
    // Retain the reading surface if verification produces a shorter answer.
    body.style.minHeight = `${body.getBoundingClientRect().height}px`;
    prior.current = {
      id: presented?.id ?? "",
      count: presented?.messages.length ?? 0,
      top: box.scrollTop,
    };
  }, [presented, collapsed]);
  const status = busy
    ? `${busy}…`
    : error || (chat ? chatProgress(chat) : profile?.availabilityReason || "");
  return (
    <FloatingPanel
      position={floating}
      class={`ai-chat${collapsed ? " ai-chat--collapsed" : ""}`}
      label={`Article chat: ${chat?.title ?? target?.title ?? "DeepSeek"}`}
      onClose={controller.close}
    >
      <header class="ai-chat__header">
        <div
          class="ai-chat__handle"
          role="button"
          tabIndex={0}
          aria-label="Move chat with arrow keys or drag"
          {...floating.handle}
        >
          <small>DeepSeek</small>
          <strong title={chat?.title ?? target?.title}>
            {chat?.title ?? target?.title}
          </strong>
        </div>
        <button
          class="icon-button"
          aria-label="New summary"
          title="New summary"
          disabled={
            !!busy ||
            active ||
            controller.unresolved ||
            !profile?.enabled ||
            !chat
          }
          onClick={controller.regenerate}
        >
          <Icon name="refresh" size={16} />
        </button>
        <button
          class="icon-button"
          aria-label={collapsed ? "Expand chat" : "Minimize chat"}
          title={collapsed ? "Expand" : "Minimize"}
          aria-expanded={!collapsed}
          onClick={() => controller.setCollapsed(!collapsed)}
        >
          <span aria-hidden="true">{collapsed ? "□" : "−"}</span>
        </button>
        <button
          class="icon-button"
          aria-label="Close chat"
          title="Close"
          onClick={controller.close}
        >
          <Icon name="close" />
        </button>
      </header>
      <div class="ai-chat__content" hidden={collapsed}>
        <div
          class="ai-chat__messages"
          ref={messages}
          onScroll={() => {
            prior.current.top = messages.current?.scrollTop ?? 0;
            onScroll();
          }}
          aria-label="Conversation messages"
        >
          <div class="ai-chat__thread" ref={thread}>
            {!presented && (
              <p class="ai-chat__empty">
                {busy
                  ? "Opening…"
                  : !profile?.enabled
                    ? "Add your DeepSeek API key in Profile to summarize articles."
                    : "Open an article to start."}
              </p>
            )}
            {presented?.messages.map((message) => {
              const content = visibleMessageContent(message);
              const pending = ["pending", "streaming"].includes(message.status);
              return (
                <section
                  key={message.id}
                  class={`ai-message ai-message--${message.role}`}
                  aria-label={
                    message.role === "assistant"
                      ? "DeepSeek response"
                      : "Your message"
                  }
                >
                  <div class="ai-message__meta">
                    <span>
                      {message.role === "user"
                        ? "You"
                        : message.purpose === "summary"
                          ? message.status === "complete"
                            ? "Summary"
                            : content
                              ? "Summary · not yet checked"
                              : "Summary"
                          : "DeepSeek"}
                    </span>
                    <CopyButton
                      text={content}
                      label={
                        message.role === "assistant"
                          ? "Copy response"
                          : "Copy message"
                      }
                    />
                  </div>
                  {content ? (
                    <ChatMarkdown text={content} />
                  ) : (
                    <p class="ai-message__placeholder">
                      {pending && (
                        <span
                          class="spinner"
                          aria-label="Waiting for response"
                        />
                      )}
                      {messagePlaceholder(message)}
                    </p>
                  )}
                </section>
              );
            })}
          </div>
        </div>
        <div class="ai-chat__status-row">
          <StatusRegion
            class="ai-chat__status"
            busy={!!busy || active}
            title={error || chat?.error || status}
          >
            {(busy || active) && <span class="spinner" aria-hidden="true" />}
            <span class={error || chat?.error ? "ai-error" : ""}>{status}</span>
          </StatusRegion>
          <div class="ai-chat__recovery">
            <button
              class="icon-button"
              style={{
                visibility:
                  active || busy === "Stopping" ? "visible" : "hidden",
              }}
              disabled={!!busy || !active}
              aria-label="Stop"
              title="Stop"
              onClick={controller.stop}
            >
              <Icon name="stop" size={16} />
            </button>
            <button
              class="icon-button"
              style={{
                visibility:
                  canRetry || busy.startsWith("Retrying")
                    ? "visible"
                    : "hidden",
              }}
              disabled={!!busy || !canRetry || !profile?.enabled}
              aria-label="Retry"
              title="Retry"
              onClick={controller.retry}
            >
              <Icon name="refresh" size={16} />
            </button>
            <button
              class="icon-button"
              style={{
                visibility:
                  controller.pollFailed ||
                  (!!error && !canRetry) ||
                  busy === "Reconnecting"
                    ? "visible"
                    : "hidden",
              }}
              disabled={!!busy}
              aria-label="Reconnect"
              title="Reconnect"
              onClick={controller.reconnect}
            >
              <Icon name="refresh" size={16} />
            </button>
          </div>
        </div>
        <form
          class="ai-chat__composer"
          onSubmit={(event) => {
            event.preventDefault();
            if (canSend) controller.send();
          }}
        >
          <AutofillResistantTextarea
            aria-label="Message DeepSeek"
            rows={2}
            value={controller.draft}
            disabled={!chat}
            placeholder="Ask about this article…"
            onInput={(event) => controller.setDraft(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
                event.preventDefault();
                if (canSend) controller.send();
              }
            }}
          />
          <button
            class="primary-button"
            type="submit"
            aria-label="Send"
            title={
              active
                ? "Available after checking · your draft is kept"
                : "Send · Ctrl / ⌘ + Enter"
            }
            disabled={!canSend}
            aria-busy={busy === "Sending message"}
          >
            {busy === "Sending message" ? (
              <span class="spinner" />
            ) : (
              <Icon name="send" size={18} />
            )}
          </button>
        </form>
      </div>
    </FloatingPanel>
  );
}
