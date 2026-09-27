import type { ArticleChat, ChatMessage } from "../api/ai";

/** A summary is publishable only after its verification has completed. Failed or
 * interrupted drafts are retained server-side, but cannot be read or copied here. */
export const visibleMessageContent = (message: ChatMessage) =>
  message.purpose === "summary" && message.status !== "complete" ? "" : message.content;

export const hasVerifiedSummary = (chat: ArticleChat | null) =>
  !!chat?.messages.some(message => message.role === "assistant" && message.purpose === "summary" && message.status === "complete");

export function lastAssistant(chat: ArticleChat): ChatMessage | undefined {
  for (let index = chat.messages.length - 1; index >= 0; index--) {
    if (chat.messages[index].role === "assistant") return chat.messages[index];
  }
}

export function chatProgress(chat: ArticleChat): string {
  const assistant = lastAssistant(chat);
  const summary = assistant?.purpose === "summary";
  const verification = assistant?.phase === "verifying";
  switch (chat.status) {
    case "waiting_content": return "Waiting for the full article. You can cancel with Stop.";
    case "queued": return verification ? "Verification queued…" : "Summary queued…";
    case "generating": return summary ? "Step 1 of 2 · Generating the summary…" : "DeepSeek is writing…";
    case "verifying": return "Step 2 of 2 · Checking the summary against the article…";
    case "completed": return "Conversation saved";
    case "failed": return verification ? "Verification failed. Retry checks the saved draft with a new provider request." : "Generation failed. Retry is an explicit new provider request.";
    case "interrupted": return verification ? "Verification interrupted. The summary is not ready; Retry checks the saved draft." : summary ? "Summary generation interrupted. No verified summary is ready." : "Response interrupted. The unfinished answer is saved.";
    case "cancelled": return verification ? "Stopped during verification. No verified summary is ready; provider charges may still apply." : "Stopped. Your conversation is saved; provider charges may still apply.";
  }
}

export function messagePlaceholder(message: ChatMessage): string {
  if (message.purpose === "summary") {
    if (["pending", "streaming"].includes(message.status)) return message.phase === "verifying"
      ? "Checking facts against the article. The summary will appear after verification."
      : "Preparing the summary. It will be checked before it appears here.";
    if (message.status !== "complete") return "No verified summary is available for this attempt.";
  }
  return "No response text was received.";
}
