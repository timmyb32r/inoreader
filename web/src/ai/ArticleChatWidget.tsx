import { useLayoutEffect, useRef, useState } from "preact/hooks";
import { isChatActive, type AiProfile } from "../api/ai";
import { AutofillResistantSelect, AutofillResistantTextarea } from "../ui/fields";
import { Icon } from "../ui/Icon";
import { formatArticleDate } from "../ui/formatArticleDate";
import { ChatMarkdown } from "./ChatMarkdown";
import { ChatUsage } from "./ChatUsage";
import { chatProgress, hasVerifiedSummary, messagePlaceholder, visibleMessageContent } from "./chatPresentation";
import type { ArticleChatController } from "./useArticleChat";
import { useFloatingChat } from "./useFloatingChat";
import "./deepseek.css";

export function ArticleChatWidget({ controller, profile, onProfile }: {
  controller: ArticleChatController;
  profile: AiProfile | null;
  onProfile: () => void;
}) {
  const { chat, target, collapsed, busy, error } = controller;
  const floating = useFloatingChat(collapsed);
  const messages = useRef<HTMLDivElement>(null);
  const followBottom = useRef(true);
  const [copyStatus, setCopyStatus] = useState("");
  const active = isChatActive(chat);
  const canRetry = controller.retryable || (!!chat && ["failed", "interrupted", "cancelled"].includes(chat.status));
  const summaryReady = hasVerifiedSummary(chat);
  const canSend = summaryReady && !!profile?.enabled && !busy && !active && !controller.unresolved && !!controller.draft.trim();
  useLayoutEffect(() => {
    if (!collapsed) floating.element.current?.querySelector<HTMLElement>("textarea:not(:disabled)")?.focus();
  }, [collapsed, chat?.id]);
  useLayoutEffect(() => {
    if (followBottom.current && messages.current) messages.current.scrollTop = messages.current.scrollHeight;
  }, [chat?.messages, collapsed]);
  const copy = async (text: string) => {
    try { await navigator.clipboard.writeText(text); setCopyStatus("Copied to clipboard"); }
    catch { setCopyStatus("Could not copy. Select the text and copy it manually."); }
  };
  return <div ref={floating.element} class={`ai-chat${collapsed ? " ai-chat--collapsed" : ""}`} style={floating.style} role="dialog" aria-modal="false" aria-label={`Article chat: ${chat?.title ?? target?.title ?? "DeepSeek"}`} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); controller.close(); } }}>
    <header class="ai-chat__header">
      <div class="ai-chat__handle" role="button" tabIndex={0} aria-label="Move chat with arrow keys or drag" {...floating.handle}>
        <small>DeepSeek · article chat</small><strong title={chat?.title ?? target?.title}>{chat?.title ?? target?.title}</strong>
      </div>
      <button class="icon-button" aria-label={collapsed ? "Expand chat" : "Minimize chat"} aria-expanded={!collapsed} onClick={() => controller.setCollapsed(!collapsed)}><span aria-hidden="true">{collapsed ? "□" : "−"}</span></button>
      <button class="icon-button" aria-label="Close chat" onClick={controller.close}><Icon name="close"/></button>
    </header>
    <div class="ai-chat__content" hidden={collapsed}>
      <div class="ai-chat__conversation">
      <div class="ai-chat__versions">
        <label class="sr-only" for="ai-chat-version">Summary version</label>
        <AutofillResistantSelect id="ai-chat-version" value={chat?.id ?? ""} disabled={!!busy || !controller.versions.length} onChange={event => controller.selectVersion(event.currentTarget.value)}>
          {!chat && <option value="">Saved conversations</option>}
          {controller.versions.map((value, index) => <option key={value.id} value={value.id}>v{controller.versions.length - index} · {formatArticleDate(value.createdAt)}{index === 0 ? " · latest" : ""}</option>)}
        </AutofillResistantSelect>
        <button class="text-button" disabled={!!busy || active || controller.unresolved || !profile?.enabled || !chat} onClick={controller.regenerate}>New summary</button>
      </div>
      <div class="ai-chat__messages" ref={messages} onScroll={() => { const box = messages.current; if (box) followBottom.current = box.scrollHeight - box.scrollTop - box.clientHeight < 48; }} aria-label="Conversation messages">
        {!chat && <div class="ai-chat__empty">{busy ? <><span class="spinner"/> Opening your article chat…</> : <><p>Summaries use the full article and your personal style prompt.</p><button class="secondary-button" onClick={onProfile}>Open DeepSeek profile</button></>}</div>}
        {chat?.messages.map(message => {
          const content = visibleMessageContent(message);
          const pending = ["pending", "streaming"].includes(message.status);
          return <section key={message.id} class={`ai-message ai-message--${message.role}`} aria-label={message.role === "assistant" ? "DeepSeek response" : "Your message"}>
          <div class="ai-message__meta"><strong>{message.role === "assistant" ? "DeepSeek" : "You"}</strong><span>{["interrupted", "failed"].includes(message.status) ? "Incomplete" : ""}</span></div>
          {content ? <ChatMarkdown text={content} onCopy={text => void copy(text)}/> : <p class="ai-message__placeholder">{pending && <span class="spinner" aria-label="Waiting for response"/>}{messagePlaceholder(message)}</p>}
          <div class="ai-message__actions"><button class="text-button" disabled={!content} onClick={() => void copy(content)}>Copy {message.role === "assistant" ? "response" : "message"}</button></div>
        </section>; })}
        {chat && !chat.messages.length && <p class="ai-chat__empty">The complete article is attached on the server. The first response will appear here.</p>}
      </div>
      <div class="ai-chat__status" role="status" aria-live="polite" aria-busy={!!busy || active}>
        {busy || active ? <span class="spinner" aria-hidden="true"/> : null}
        <span class={error || chat?.error ? "ai-error" : ""}>{busy ? `${busy}…` : error || (chat ? <>{chatProgress(chat)}{chat.error && <small>{chat.error}</small>}</> : "")}{copyStatus && <small>{copyStatus}</small>}</span>
      </div>
      <ChatUsage calls={chat?.providerCalls ?? []}/>
      <div class="ai-chat__commands">
        <button class="secondary-button" disabled={!!busy || !active} onClick={controller.stop}>Stop</button>
        <button class="secondary-button" disabled={!!busy || !canRetry || !profile?.enabled} onClick={controller.retry}>Retry</button>
        <button class="text-button" disabled={!!busy} onClick={controller.reconnect}>Reconnect</button>
        <button class="text-button" onClick={onProfile}>Profile</button>
      </div>
      </div>
      <form class="ai-chat__composer" onSubmit={event => { event.preventDefault(); if (canSend) { setCopyStatus(""); controller.send(); } }}>
        <AutofillResistantTextarea aria-label="Message DeepSeek" rows={3} value={controller.draft} disabled={!chat} placeholder="Ask about the article, request a quote or rephrase…" onInput={event => controller.setDraft(event.currentTarget.value)} onKeyDown={event => { if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) { event.preventDefault(); if (canSend) controller.send(); } }}/>
        <div><small>{!profile?.enabled ? profile?.availabilityReason || "Configure DeepSeek in Profile to send" : chat && !summaryReady ? "Chat unlocks after a verified summary. Your draft is kept; retry if needed." : "Ctrl / ⌘ + Enter to send · no web search"}</small><button class="primary-button" type="submit" aria-label="Send" disabled={!canSend} aria-busy={busy === "Sending message"}>{busy === "Sending message" ? <span class="spinner"/> : "Send"}</button></div>
      </form>
    </div>
  </div>;
}
