import type { Transport } from "./client";
export class SearchClient {
  constructor(private readonly transport: Transport) {}
  limits = () =>
    this.transport("/api/search/limits", undefined, "SearchLimitsView");
  query = (params: URLSearchParams) =>
    this.transport(`/api/search?${params}`, undefined, "SearchPage");
}
