import type { Transport } from "./client";
import type {
  WikiWrite,
  WikiDraft,
  WikiMemberCommand,
  WikiBindCommand,
} from "./generated";
const body = (method: string, value: unknown): RequestInit => ({
  method,
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(value),
});
export class WikiClient {
  constructor(private readonly transport: Transport) {}
  links = (ns: string, id: string) =>
    this.transport(
      `/api/wiki/${ns}/pages/${id}/links`,
      undefined,
      "WikiLink[]",
    );
  revision = (ns: string, id: string, revision: string) =>
    this.transport(
      `/api/wiki/${ns}/pages/${id}/history/${revision}`,
      undefined,
      "WikiRevision",
    );
  limits = () => this.transport("/api/wiki/limits", undefined, "WikiLimits");
  namespaces = (offset = 0) =>
    this.transport(
      `/api/wiki/namespaces?offset=${offset}`,
      undefined,
      "WikiNamespaces",
    );
  createNamespace = (id: string, name: string) =>
    this.transport(
      "/api/wiki/namespaces",
      body("POST", { id, name }),
      "WikiNamespace",
    );
  namespace = (ns: string) =>
    this.transport(`/api/wiki/${ns}`, undefined, "WikiNamespace");
  pages = (ns: string, search = "", trash = false, offset = 0) =>
    this.transport(
      `/api/wiki/${ns}/pages?${new URLSearchParams({ search, trash: String(trash), offset: String(offset) })}`,
      undefined,
      "WikiPages",
    );
  page = (ns: string, id: string) =>
    this.transport(`/api/wiki/${ns}/pages/${id}`, undefined, "WikiPage");
  resolve = (ns: string, name: string) =>
    this.transport(
      `/api/wiki/${ns}/resolve?${new URLSearchParams({ name })}`,
      undefined,
      "WikiPage",
    );
  write = (ns: string, command: WikiWrite) =>
    this.transport(`/api/wiki/${ns}/pages`, body("POST", command), "WikiPage");
  history = (ns: string, id: string, offset = 0) =>
    this.transport(
      `/api/wiki/${ns}/pages/${id}/history?offset=${offset}`,
      undefined,
      "WikiHistory",
    );
  draft = (ns: string, id: string) =>
    this.transport(
      `/api/wiki/${ns}/drafts/${id}`,
      undefined,
      "WikiDraftResponse",
    );
  saveDraft = (ns: string, draft: WikiDraft) =>
    this.transport(
      `/api/wiki/${ns}/drafts/${draft.id}`,
      body("PUT", draft),
      "WikiDraft",
    );
  discard = (ns: string, id: string, revision: string | null) =>
    this.transport(
      `/api/wiki/${ns}/drafts/${id}`,
      body("DELETE", { revision }),
      "empty",
    );
  members = (ns: string, offset = 0) =>
    this.transport(
      `/api/wiki/${ns}/members?offset=${offset}`,
      undefined,
      "WikiMembers",
    );
  setMember = (ns: string, command: WikiMemberCommand) =>
    this.transport(`/api/wiki/${ns}/members`, body("PUT", command), "empty");
  binding = (subscription: string) =>
    this.transport(
      `/api/subscriptions/${subscription}/wiki`,
      undefined,
      "WikiBinding",
    );
  bind = (subscription: string, command: WikiBindCommand) =>
    this.transport(
      `/api/subscriptions/${subscription}/wiki`,
      body("PUT", command),
      "empty",
    );
}
