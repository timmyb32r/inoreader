import type { ArticleChat, ChatMessage } from "../api/ai";

/** The API projects a complete first-pass preview while verification is pending.
 * Partial verifier output is never returned. Status distinguishes it from final. */
export const visibleMessageContent = (message: ChatMessage) => message.content;

export const hasVerifiedSummary = (chat: ArticleChat | null) =>
  !!chat?.messages.some(
    (message) =>
      message.role === "assistant" &&
      message.purpose === "summary" &&
      message.status === "complete",
  );

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
    case "waiting_content":
      return "Waiting for the full article…";
    case "queued":
      return verification ? "Verification queued…" : "Summary queued…";
    case "generating":
      return summary ? "Writing summary…" : "DeepSeek is writing…";
    case "verifying":
      return "Checking facts…";
    case "completed":
      return "";
    case "failed":
      return verification
        ? "Verification failed · preview kept. Retry to check again."
        : "Generation failed · retry to try again.";
    case "interrupted":
      return verification
        ? "Verification interrupted · preview kept."
        : summary
          ? "Summary interrupted."
          : "Response interrupted.";
    case "cancelled":
      return verification
        ? "Stopped during verification · preview kept."
        : "Stopped.";
  }
}

export function messagePlaceholder(message: ChatMessage): string {
  if (message.purpose === "summary") {
    if (["pending", "streaming"].includes(message.status))
      return message.phase === "verifying"
        ? "Checking facts…"
        : "Writing summary…";
    if (message.status !== "complete")
      return "No summary is available for this attempt.";
  }
  return "No response text was received.";
}
