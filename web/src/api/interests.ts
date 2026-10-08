import type { Transport } from "./client";
import type { InterestProfile } from "./generated";
export class InterestClient {
  constructor(private readonly transport: Transport) {}
  feed = (workspace: string, cursor?: string, showHidden = false) =>
    this.transport(
      `/api/ai/smart-feed?${new URLSearchParams({ workspace_id: workspace, show_hidden: String(showHidden), ...(cursor ? { cursor } : {}) })}`,
      undefined,
      "SmartFeed",
    );
  random = (workspace: string, seed: string, cursor?: string) =>
    this.transport(
      `/api/ai/random-feed?${new URLSearchParams({ workspace_id: workspace, seed, ...(cursor ? { cursor } : {}) })}`,
      undefined,
      "SmartFeed",
    );
  save = (profile: InterestProfile) =>
    this.transport(
      "/api/ai/interests",
      {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(profile),
      },
      "InterestProfile",
    );
}
