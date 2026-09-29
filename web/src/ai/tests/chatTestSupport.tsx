import { AiClient, type AiProfile, type ArticleChat } from "../../api/ai";
import { ArticleChatWidget } from "../ArticleChatWidget";
import { useArticleChat } from "../useArticleChat";

export const profile: AiProfile = {
  models: { summary: "deepseek-flash", verification: "deepseek-flash" },
  configured: true,
  enabled: true,
};
export const saved = (id = "chat1", articleId = "a"): ArticleChat => ({
  id,
  articleId,
  workspaceId: "ws",
  title: `Article ${articleId}`,
  sourceUrl: "https://example.test",
  createdAt: "2026-09-26T12:00:00Z",
  model: "fixture",
  promptVersion: "1",
  status: "completed",
  providerCalls: [],
  messages: [
    {
      id: "m1",
      role: "assistant",
      purpose: "summary",
      phase: "verifying",
      status: "complete",
      content: "**Useful summary**",
      createdAt: "2026-09-26T12:00:00Z",
    },
  ],
});
export function Harness({
  client,
  configured = profile,
  account = "account",
}: {
  client: AiClient;
  configured?: AiProfile;
  account?: string;
}) {
  const controller = useArticleChat(client, account, configured);
  return (
    <>
      <button
        onClick={() =>
          void controller.open({
            articleId: "a",
            workspaceId: "ws",
            title: "Article a",
          })
        }
      >
        Summarize A
      </button>
      <button
        onClick={() =>
          void controller.open({
            articleId: "b",
            workspaceId: "ws",
            title: "Article b",
          })
        }
      >
        Summarize B
      </button>
      {controller.visible && (
        <ArticleChatWidget controller={controller} profile={configured} />
      )}
    </>
  );
}
