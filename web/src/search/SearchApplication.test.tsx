import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/preact";
import { afterEach, expect, it, vi } from "vitest";
import { useState } from "preact/hooks";
import { ApiClient, type Transport } from "../api/client";
import { SearchApplication } from "./SearchApplication";
afterEach(cleanup);
it("deduplicates submissions, keeps scoped URL, ignores late results and preserves preview on selection", async () => {
  let resolveFirst: (value: unknown) => void = () => {};
  const requests: string[] = [];
  const transport = vi.fn((url: string) => {
    if (url === "/api/search/limits")
      return Promise.resolve({
        query_bytes: 512,
        page_size: 25,
        excerpt_characters: 220,
      });
    requests.push(url);
    if (url.includes("q=slow") && url.includes("kind=wiki"))
      return new Promise((resolve) => {
        resolveFirst = resolve;
      });
    return Promise.resolve({ items: [], has_more: false });
  }) as unknown as Transport;
  const client = new ApiClient(transport);
  function Harness() {
    const [url, setUrl] = useState("/search?kind=wiki&namespace=private");
    return (
      <SearchApplication
        client={client}
        url={url}
        navigate={(u) => {
          setUrl(u);
          return true;
        }}
      />
    );
  }
  render(<Harness />);
  await waitFor(() =>
    expect(transport).toHaveBeenCalledWith(
      "/api/search/limits",
      undefined,
      "SearchLimitsView",
    ),
  );
  const input = screen.getByRole("searchbox");
  fireEvent.input(input, { target: { value: "slow" } });
  const submit = screen.getByRole("button", { name: "Search", exact: true });
  fireEvent.submit(submit.closest("form")!);
  fireEvent.submit(submit.closest("form")!);
  await waitFor(() => expect(requests).toHaveLength(1));
  expect(requests[0]).toContain("namespace=private");
  expect(submit).toHaveAttribute("aria-busy", "true");
  expect(submit).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "News", exact: true }));
  await waitFor(() => expect(requests).toHaveLength(2));
  expect(requests[1]).not.toContain("namespace");
  // A later query wins even if the older request finishes last.
  fireEvent.click(screen.getByRole("button", { name: "All", exact: true }));
  await waitFor(() => expect(requests).toHaveLength(3));
  await waitFor(() => expect(submit).not.toBeDisabled());
  resolveFirst({
    items: [
      {
        title: "Stale result",
        target: { kind: "news", workspace: "w", article: "a" },
        context: "old",
        excerpt: "old",
        updated_at: "old",
      },
    ],
    has_more: false,
  });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(screen.queryByText("Stale result")).toBeNull();
});
it("renders server excerpts as text and fetches preview through its authorized API", async () => {
  const id = "00000000-0000-0000-0000-000000000001";
  const transport = vi.fn((url: string) =>
    Promise.resolve(
      url.includes("limits")
        ? { query_bytes: 512, page_size: 25, excerpt_characters: 220 }
        : url.startsWith("/api/search?")
          ? {
              items: [
                {
                  title: "Private page",
                  context: "Engineering",
                  excerpt: "<img src=x onerror=alert(1)>",
                  updated_at: "today",
                  target: { kind: "wiki", namespace: id, page: id },
                },
              ],
              has_more: false,
            }
          : {
              namespace: id,
              id,
              name: "Private page",
              markdown: "Authorized content",
              deleted: false,
            },
    ),
  ) as unknown as Transport;
  function Harness() {
    const [url, setUrl] = useState("/search?q=private&kind=all");
    return (
      <SearchApplication
        client={newClient}
        url={url}
        navigate={(u) => {
          setUrl(u);
          return true;
        }}
      />
    );
  }
  const newClient = new ApiClient(transport);
  const { container } = render(<Harness />);
  const hit = await screen.findByRole("button", { name: /Private page/ });
  expect(container.querySelector("img")).toBeNull();
  fireEvent.click(hit);
  await screen.findByText("Authorized content");
  expect(transport).toHaveBeenCalledWith(
    `/api/wiki/${id}/pages/${id}`,
    undefined,
    "WikiPage",
  );
});
it("shows a failed wiki link in the reserved preview status without removing its page", async () => {
  const id = "00000000-0000-0000-0000-000000000001";
  const transport = vi.fn((url: string) =>
    url.includes("/resolve?")
      ? Promise.reject(new Error("Wiki page not found"))
      : Promise.resolve(
          url.includes("limits")
            ? { query_bytes: 512, page_size: 25, excerpt_characters: 220 }
            : url.startsWith("/api/search?")
              ? { items: [], has_more: false }
              : {
                  namespace: id,
                  id,
                  name: "Selected page",
                  markdown: "[[Missing]]",
                  deleted: false,
                },
        ),
  ) as unknown as Transport;
  render(
    <SearchApplication
      client={new ApiClient(transport)}
      url={`/search?q=selected&kind=wiki&selectedKind=wiki&container=${id}&selected=${id}`}
      navigate={() => true}
    />,
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Missing", exact: true }),
  );
  await screen.findByText("Wiki page not found");
  expect(screen.getByRole("heading", { name: "Selected page" })).toBeVisible();
});
