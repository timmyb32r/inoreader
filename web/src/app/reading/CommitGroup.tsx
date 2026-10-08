import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../../api/client";
import type { Article } from "../../api/viewModels";
import type { ArticleChat } from "../../api/ai";
import { AsyncButton } from "../../ui/AsyncButton";
import { ChatMarkdown } from "../../ai/ChatMarkdown";
import { commitProject } from "./commitArticle";
import "./commit-group.css";

/** Use the authored TL;DR paragraph, without clipping sentences or inventing content. */
export function commitTldr(text: string): string {
  const blocks = text.split(/\n\s*\n/).filter((block) => block.trim());
  const index = blocks.findIndex((block) => /(?:TL;?DR|TL;DR)\b/i.test(block));
  if (index >= 0) {
    const paragraph = blocks[index].replace(
      /^\s*(?:#{1,6}\s*)?\*{0,2}TL;?DR\*{0,2}\s*[:—-]?\s*\*{0,2}\s*/i,
      "",
    );
    return paragraph.trim() ? paragraph : (blocks[index + 1] ?? "");
  }
  return blocks.find((block) => !/^\s*#/.test(block)) ?? "";
}
function chatSummary(chat: ArticleChat): string {
  return (
    chat.messages
      .filter(
        (message) =>
          message.role === "assistant" &&
          message.purpose === "summary" &&
          message.content,
      )
      .at(-1)?.content ?? ""
  );
}
type Summary = {
  text: string;
  status: string;
  pending?: boolean;
  retryChat?: string;
};
/** One explicit snapshot; pages never impose a silent 50-commit cutoff. Each row
 * reserves its paragraph slot, so generation/polling cannot move its controls. */
export function CommitGroup({
  client,
  workspace,
  article,
  enabled,
  busy,
  onSnapshot,
  onDiscuss,
  failures,
}: {
  client: ApiClient;
  workspace: string;
  article: Article;
  enabled: boolean;
  busy: boolean;
  onSnapshot: (ids: string[] | null) => void;
  onDiscuss: (article: Article) => void;
  failures: string[];
}) {
  const [items, setItems] = useState<Article[]>([]);
  const [summaries, setSummaries] = useState<Record<string, Summary>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  const intents = useRef(new Map<string, string>());
  const watch = useRef<(id: string, chat: ArticleChat) => void>(() => {});
  const scope = JSON.stringify([workspace, article.id]);
  const currentScope = useRef(scope);
  currentScope.current = scope;
  useEffect(() => {
    let active = true;
    const pending = new Map<string, { article: string; chat: string }>();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const publish = (id: string, chat: ArticleChat) => {
      if (!active) return;
      const text = commitTldr(chatSummary(chat));
      setSummaries((previous) => ({
        ...previous,
        [id]: {
          text,
          retryChat: ["failed", "cancelled", "interrupted"].includes(
            chat.status,
          )
            ? chat.id
            : undefined,
          pending: [
            "waiting_content",
            "queued",
            "generating",
            "verifying",
          ].includes(chat.status),
          status:
            chat.error ||
            (["failed", "cancelled"].includes(chat.status)
              ? "Подготовка TL;DR не завершена; откройте коммит для восстановления."
              : text
                ? ""
                : "TL;DR готовится в пределах AI-бюджета"),
        },
      }));
      if (
        ["waiting_content", "queued", "generating", "verifying"].includes(
          chat.status,
        )
      )
        pending.set(id, { article: id, chat: chat.id });
      else pending.delete(id);
    };
    const poll = async () => {
      const jobs = [...pending.values()];
      let index = 0;
      await Promise.all(
        Array.from({ length: Math.min(4, jobs.length) }, async () => {
          while (active && index < jobs.length) {
            const job = jobs[index++];
            try {
              publish(job.article, await client.ai.get(job.chat));
            } catch (cause) {
              if (active)
                setSummaries((previous) => ({
                  ...previous,
                  [job.article]: {
                    ...previous[job.article],
                    status: (cause as Error).message,
                  },
                }));
              pending.delete(job.article);
            }
          }
        }),
      );
      if (active && pending.size) timer = setTimeout(() => void poll(), 5000);
    };
    watch.current = (id, chat) => {
      publish(id, chat);
      if (active && pending.size) {
        clearTimeout(timer);
        timer = setTimeout(() => void poll(), 5000);
      }
    };
    onSnapshot(null);
    setLoading(true);
    setError("");
    setItems([]);
    setSummaries({});
    void (async () => {
      try {
        const collected: Article[] = [],
          seen = new Set<string>();
        let cursor: string | undefined;
        do {
          const page = await client.reading.commits(
            workspace,
            article.id,
            cursor,
          );
          if (!active) return;
          for (const item of page.articles) {
            if (!seen.has(item.id)) {
              seen.add(item.id);
              collected.push(item);
            }
          }
          const next = page.olderCursor ?? undefined;
          if (next && next === cursor)
            throw Error("Commit pagination did not advance");
          cursor = next;
        } while (cursor);
        if (!active) return;
        setItems(collected);
        setLoading(false);
        onSnapshot(collected.map((item) => item.id));
        let index = 0;
        await Promise.all(
          Array.from({ length: Math.min(4, collected.length) }, async () => {
            while (active && index < collected.length) {
              const item = collected[index++];
              try {
                const chats = await client.ai.versions(workspace, item.id);
                if (!active) return;
                const latest = [...chats].sort(
                  (a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt),
                )[0];
                if (latest && chatSummary(latest)) publish(item.id, latest);
                else if (latest) {
                  publish(item.id, latest);
                } else
                  setSummaries((previous) => ({
                    ...previous,
                    [item.id]: {
                      text: "",
                      status: "Готового TL;DR нет. Запустите пересказ явно.",
                    },
                  }));
              } catch (cause) {
                if (active)
                  setSummaries((previous) => ({
                    ...previous,
                    [item.id]: { text: "", status: (cause as Error).message },
                  }));
              }
            }
          }),
        );
        if (active && pending.size) timer = setTimeout(() => void poll(), 5000);
      } catch (cause) {
        if (active) {
          setError((cause as Error).message);
          setLoading(false);
        }
      }
    })();
    return () => {
      active = false;
      if (timer) clearTimeout(timer);
    };
  }, [client, workspace, article.id, enabled, reload]);
  return (
    <section
      class="commit-group"
      aria-label="Непрочитанные коммиты проекта"
      aria-busy={loading}
    >
      <header>
        <h1>{commitProject(article.url)} · коммиты</h1>
        <div class="commit-group__status" role="status">
          {error ||
            (loading
              ? "Загружаю все непрочитанные коммиты…"
              : `${items.length} непрочитанных коммитов`)}
        </div>
      </header>
      {error && (
        <button
          class="secondary-button"
          disabled={busy}
          onClick={() => setReload((value) => value + 1)}
        >
          Повторить загрузку коммитов
        </button>
      )}
      {items.map((item) => (
        <article
          class="commit-group__item"
          key={item.id}
          aria-label={item.title || "Коммит без заголовка"}
        >
          <div class="commit-group__heading">
            <h2>{item.title || "Коммит без заголовка"}</h2>
            <a href={item.url} target="_blank" rel="noopener noreferrer">
              Оригинал
            </a>
          </div>
          <div class="commit-group__paragraph" aria-label="TL;DR">
            {summaries[item.id]?.text ? (
              <ChatMarkdown text={summaries[item.id].text} />
            ) : (
              <p>{summaries[item.id]?.status || "Загружаю TL;DR…"}</p>
            )}
          </div>
          <div class="commit-group__actions">
            <button
              class="secondary-button"
              disabled={busy}
              onClick={() => onDiscuss(item)}
            >
              Обсудить с ИИ
            </button>
            <AsyncButton
              class="secondary-button"
              disabled={
                busy ||
                !enabled ||
                !!summaries[item.id]?.text ||
                !!summaries[item.id]?.pending
              }
              aria-label={`Сделать TL;DR: ${item.title}`}
              onPress={async () => {
                const intentKey = JSON.stringify([workspace, item.id]);
                const operation =
                  intents.current.get(intentKey) ?? crypto.randomUUID();
                intents.current.set(intentKey, operation);
                const retryChat = summaries[item.id]?.retryChat;
                const result = retryChat
                  ? await client.ai.retry(retryChat, operation)
                  : await client.ai.open(workspace, item.id, operation);
                intents.current.delete(intentKey);
                if (currentScope.current === scope)
                  watch.current(item.id, result);
              }}
              onError={(cause) =>
                setSummaries((previous) => ({
                  ...previous,
                  [item.id]: {
                    ...previous[item.id],
                    text: previous[item.id]?.text ?? "",
                    status: (cause as Error).message,
                  },
                }))
              }
            >
              Сделать TL;DR
            </AsyncButton>
            <span role="status">
              {failures.includes(item.id)
                ? "Не удалось отметить прочитанным. Повторите действие."
                : (summaries[item.id]?.status ?? "")}
            </span>
          </div>
        </article>
      ))}
    </section>
  );
}
