import type { Transport } from "./client";
export class ZhihuClient {
  constructor(private readonly transport: Transport) {}
  status = () =>
    this.transport("/api/profile/zhihu", undefined, "ZhihuProfileView");
  save = (cookies: string) =>
    this.transport(
      "/api/profile/zhihu",
      {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ cookies }),
      },
      "ZhihuProfileView",
    );
  check = () =>
    this.transport(
      "/api/profile/zhihu/check",
      { method: "POST" },
      "ZhihuProfileView",
    );
  remove = () =>
    this.transport(
      "/api/profile/zhihu",
      { method: "DELETE" },
      "ZhihuProfileView",
    );
}
