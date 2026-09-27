import type { Transport } from "./client";
export type {
  ChannelStatus,
  StyledText,
  EntityDefinition,
  KnownDefinition,
  DefinitionsJob,
  DefinitionsView,
} from "./generated";
export type { MarkKind as MarkStyle } from "./generated";
import type { ChannelStatus, DefinitionsView } from "./generated";
const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});
export class GlossaryClient {
  constructor(private readonly transport: Transport) {}
  status = (workspace: string) =>
    this.transport(
      `/api/glossary/channel?workspace_id=${encodeURIComponent(workspace)}`,
      undefined,
      "ChannelStatus",
    );
  configure = (workspaceId: string, token: string) =>
    this.transport(
      "/api/glossary/channel",
      json("PUT", { workspaceId, token }),
      "ChannelStatus",
    );
  sync = (workspaceId: string) =>
    this.transport(
      "/api/glossary/channel/sync",
      json("POST", { workspaceId }),
      "ChannelStatus",
    );
  get = (workspace: string, article: string) =>
    this.transport(
      `/api/articles/${encodeURIComponent(article)}/definitions?workspace_id=${encodeURIComponent(workspace)}`,
      undefined,
      "DefinitionsView",
    );
  generate = (
    workspaceId: string,
    article: string,
    operationId: string,
    regenerate = false,
  ) =>
    this.transport(
      `/api/articles/${encodeURIComponent(article)}/definitions`,
      json("POST", { workspaceId, operationId, regenerate }),
      "DefinitionsView",
    );
}
