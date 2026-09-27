import { useEffect, useRef, useState } from "preact/hooks";
import {
  isChatActive,
  type AiClient,
  type AiProfile,
  type ArticleChat,
} from "../api/ai";
import { ApiError } from "../api/client";
import { hasVerifiedSummary, lastAssistant } from "./chatPresentation";

export type ChatTarget = {
  articleId: string;
  workspaceId: string;
  title: string;
};
type PendingAction = { run: () => Promise<ArticleChat>; after?: () => void };
type UnresolvedRequest = {
  action: PendingAction;
  error: string;
  uncertain: boolean;
};
const targetScope = (target: ChatTarget) =>
  JSON.stringify([target.workspaceId, target.articleId]);
const orderedVersions = (values: ArticleChat[]) =>
  [...values].sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));

/** An explicit button owns each paid action. Opening or reconnecting an existing
 * conversation only reads its persisted state. Drafts are account-local memory. */
export function useArticleChat(
  client: AiClient,
  accountId: string,
  profile: AiProfile | null,
) {
  const [target, setTarget] = useState<ChatTarget | null>(null);
  const [chat, setChat] = useState<ArticleChat | null>(null);
  const [visible, setVisible] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [retryable, setRetryable] = useState(false);
  const [pollFailed, setPollFailed] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [completedGeneration, setCompletedGeneration] = useState(0);
  const lock = useRef(false);
  const epoch = useRef(0);
  const revision = useRef(0);
  // Keep operation closures across read/reopen/article switches until their exact
  // operation ID receives an acknowledgement. A GET cannot resolve a lost POST.
  const pendingRequests = useRef(new Map<string, UnresolvedRequest>());
  const trigger = useRef<HTMLElement | null>(null);
  const current = useRef(chat);
  current.current = chat;
  const draft = chat ? (drafts[chat.id] ?? "") : "";
  const scope = target ? targetScope(target) : "";
  const unresolved = pendingRequests.current.get(scope)?.uncertain ?? false;

  useEffect(() => {
    epoch.current += 1;
    lock.current = false;
    pendingRequests.current.clear();
    setTarget(null);
    setChat(null);
    setDrafts({});
    setVisible(false);
    setBusy("");
    setError("");
    setRetryable(false);
    setPollFailed(false);
    setCompletedGeneration(0);
  }, [accountId]);
  useEffect(
    () => () => {
      epoch.current += 1;
    },
    [],
  );

  const accept = (next: ArticleChat) => {
    if (
      current.current?.id === next.id &&
      isChatActive(current.current) &&
      next.status === "completed"
    )
      setCompletedGeneration((value) => value + 1);
    current.current = next;
    setChat(next);
  };
  const command = async (
    label: string,
    action: PendingAction,
    actionScope = scope,
    billable = true,
  ) => {
    const prior = pendingRequests.current.get(actionScope);
    if (
      lock.current ||
      (billable && prior?.uncertain && prior.action !== action)
    )
      return;
    lock.current = true;
    revision.current += 1;
    const token = epoch.current;
    setBusy(label);
    setError("");
    setRetryable(false);
    setPollFailed(false);
    try {
      const next = await action.run();
      if (token !== epoch.current) return;
      accept(next);
      action.after?.();
      if (billable) pendingRequests.current.delete(actionScope);
      const remaining = pendingRequests.current.get(actionScope);
      if (remaining) {
        setError(remaining.error);
        setRetryable(true);
      }
    } catch (cause) {
      if (token !== epoch.current) return;
      const rejected =
        cause instanceof ApiError &&
        cause.status >= 400 &&
        cause.status < 500 &&
        cause.status !== 408;
      const message =
        cause instanceof Error
          ? cause.message
          : "The request could not be completed. No automatic retry was sent.";
      const detail =
        billable && !rejected
          ? `${message} The request outcome is unknown. Use Retry to recover the same operation before sending anything new.`
          : message;
      if (billable)
        pendingRequests.current.set(actionScope, {
          action,
          error: detail,
          uncertain: !rejected,
        });
      setError(detail);
      setRetryable(pendingRequests.current.has(actionScope));
    } finally {
      if (token === epoch.current) {
        lock.current = false;
        setBusy("");
      }
    }
  };

  const open = async (nextTarget: ChatTarget, generateIfMissing = true) => {
    setVisible(true);
    setCollapsed(false);
    if (lock.current) return;
    trigger.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const token = ++epoch.current;
    lock.current = true;
    setTarget(nextTarget);
    setChat(null);
    setBusy("Opening chat");
    const nextScope = targetScope(nextTarget),
      pendingRequest = pendingRequests.current.get(nextScope);
    setError(pendingRequest?.error ?? "");
    setRetryable(!!pendingRequest);
    setPollFailed(false);
    try {
      const saved = orderedVersions(
        await client.versions(nextTarget.workspaceId, nextTarget.articleId),
      );
      if (token !== epoch.current) return;
      if (saved.length) {
        accept(saved[0]);
        return;
      }
      if (pendingRequest) return;
      if (!generateIfMissing) {
        setError("No saved conversation yet. Click Summarize to start one.");
        return;
      }
      if (!profile?.enabled) {
        setError(
          profile?.availabilityReason ||
            "Add and validate your DeepSeek API key in Profile to summarize this article.",
        );
        return;
      }
      lock.current = false;
      const operationId = crypto.randomUUID();
      await command(
        "Starting summary",
        {
          run: () =>
            client.open(
              nextTarget.workspaceId,
              nextTarget.articleId,
              operationId,
            ),
        },
        nextScope,
      );
    } catch (cause) {
      if (token === epoch.current)
        setError(
          cause instanceof Error
            ? cause.message
            : "Could not load saved conversations. Try opening this chat again.",
        );
    } finally {
      if (token === epoch.current) {
        lock.current = false;
        setBusy("");
      }
    }
  };

  useEffect(() => {
    if (!visible || !chat || !isChatActive(chat) || pollFailed) return;
    let stopped = false;
    let timer = 0;
    const token = epoch.current;
    const id = chat.id;
    const poll = async () => {
      if (stopped) return;
      if (document.hidden || lock.current) {
        timer = window.setTimeout(poll, 1000);
        return;
      }
      const requestRevision = revision.current;
      try {
        const next = await client.get(id);
        if (
          stopped ||
          token !== epoch.current ||
          requestRevision !== revision.current
        )
          return;
        accept(next);
        if (isChatActive(next)) timer = window.setTimeout(poll, 1000);
      } catch (cause) {
        if (stopped || token !== epoch.current) return;
        setError(
          cause instanceof Error
            ? cause.message
            : "Connection lost. Your conversation is saved.",
        );
        setPollFailed(true);
      }
    };
    timer = window.setTimeout(poll, 1000);
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [client, visible, chat?.id, chat?.status, pollFailed, busy]);

  const send = () => {
    if (
      !chat ||
      !hasVerifiedSummary(chat) ||
      !draft.trim() ||
      !profile?.enabled ||
      isChatActive(chat) ||
      lock.current ||
      pendingRequests.current.get(scope)?.uncertain
    )
      return;
    const id = chat.id,
      content = draft,
      operationId = crypto.randomUUID();
    void command("Sending message", {
      run: () => client.send(id, operationId, content),
      after: () =>
        setDrafts((values) =>
          values[id] === content ? { ...values, [id]: "" } : values,
        ),
    });
  };
  const regenerate = () => {
    if (
      !target ||
      !profile?.enabled ||
      isChatActive(chat) ||
      pendingRequests.current.get(scope)?.uncertain
    )
      return;
    const operationId = crypto.randomUUID();
    void command("Starting new summary", {
      run: () =>
        client.open(target.workspaceId, target.articleId, operationId, true),
    });
  };
  const retry = () => {
    const pendingRequest = pendingRequests.current.get(scope);
    if (pendingRequest) {
      void command("Retrying request", pendingRequest.action);
      return;
    }
    if (!chat || !profile?.enabled || isChatActive(chat)) return;
    const operationId = crypto.randomUUID();
    void command(
      lastAssistant(chat)?.phase === "verifying"
        ? "Retrying verification"
        : "Retrying response",
      { run: () => client.retry(chat.id, operationId) },
    );
  };
  const stop = () => {
    if (chat && isChatActive(chat))
      void command(
        "Stopping",
        { run: () => client.stop(chat.id) },
        scope,
        false,
      );
  };
  const reconnect = () => {
    if (!chat) {
      if (target) void open(target, false);
      return;
    }
    void command(
      "Reconnecting",
      { run: () => client.get(chat.id) },
      scope,
      false,
    );
  };
  const close = () => {
    setVisible(false);
    trigger.current?.focus();
  };
  return {
    target,
    chat,
    visible,
    collapsed,
    setCollapsed,
    busy,
    error,
    retryable,
    unresolved,
    pollFailed,
    draft,
    completedGeneration,
    open,
    close,
    send,
    regenerate,
    retry,
    stop,
    reconnect,
    setDraft: (value: string) => {
      if (chat) setDrafts((values) => ({ ...values, [chat.id]: value }));
    },
  };
}

export type ArticleChatController = ReturnType<typeof useArticleChat>;
