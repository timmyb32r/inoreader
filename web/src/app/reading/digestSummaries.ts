import type { ApiClient } from "../../api/client";
import type { Article } from "../../api/viewModels";

export type DigestSummary = { text: string; status: string; failed?: boolean };

/** Load saved text before publishing an interactive page. Four workers bound
 * concurrent GETs, without limiting or truncating the selected articles/text.
 * Superseded selections stop scheduling new requests. Never generates AI work. */
export async function loadDigestSummaries(
  client: ApiClient,
  workspace: string,
  articles: Article[],
  active: () => boolean,
): Promise<Record<string, DigestSummary>> {
  const results: Record<string, DigestSummary> = {};
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(4, articles.length) }, async () => {
      while (active() && next < articles.length) {
        const article = articles[next++];
        if (article.fullText !== "ready") {
          results[article.id] = {
            text: "",
            status: "Полный текст ещё не получен. Пересказ недоступен.",
          };
          continue;
        }
        try {
          const chats = await client.ai.versions(workspace, article.id);
          const latest = [...chats].sort(
            (a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt),
          )[0];
          const message = latest?.messages
            .filter(
              (m) =>
                m.role === "assistant" &&
                m.purpose === "summary" &&
                m.content !== "",
            )
            .at(-1);
          results[article.id] = {
            text: message?.content ?? "",
            status: !message
              ? "Готового пересказа пока нет."
              : message.status !== "complete"
                ? "Предварительный пересказ"
                : message.phase === "verifying"
                  ? "Готовый пересказ"
                  : "Пересказ без фактчека",
          };
        } catch (cause) {
          results[article.id] = {
            text: "",
            status:
              cause instanceof Error
                ? cause.message
                : "Не удалось загрузить пересказ. Обновите подборку.",
            failed: true,
          };
        }
      }
    }),
  );
  return results;
}
