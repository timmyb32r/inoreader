import type { Transport } from "./client";
import type { CompleteReading } from "./generated";
export type {
  ReadingState,
  ReadingCompletion,
  CompleteReading,
} from "./generated";
const path = (article: string) =>
  `/api/articles/${encodeURIComponent(article)}/reading`;
const query = (workspace: string) =>
  `?workspace_id=${encodeURIComponent(workspace)}`;
export class FocusedReadingClient {
  constructor(private readonly transport: Transport) {}
  state = (workspace: string, article: string) =>
    this.transport(path(article) + query(workspace), undefined, "ReadingState");
  next = (workspace: string, article: string) =>
    this.transport(
      path(article) + "/next" + query(workspace),
      undefined,
      "ArticlePageView",
    );
  complete = (workspace: string, article: string, command: CompleteReading) =>
    this.transport(
      path(article) + query(workspace),
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(command),
      },
      "ReadingCompletion",
    );
  undo = (workspace: string, article: string, operation: string) =>
    this.transport(
      path(article) +
        `/${encodeURIComponent(operation)}/undo` +
        query(workspace),
      { method: "POST" },
      "ReadingCompletion",
    );
}
