import { expect, test } from "@playwright/test";
test("wiki organization keeps controls stable, stars pages and assigns a parent", async ({
  page,
}) => {
  let current = {
    namespace: "ns",
    id: "p",
    revision: "r1",
    name: "A page",
    markdown: "Original text",
    deleted: false,
    author: "a",
    updated_at: "2026-09-29T00:00:00Z",
    parent: null as string | null,
  };
  let starred = false;
  let writes = 0;
  let release!: () => void;
  const pending = new Promise<void>((r) => (release = r));
  const summary = (id: string, name: string) => ({
    id,
    name,
    updated_at: current.updated_at,
    excerpt: "",
    excerpt_truncated: false,
  });
  await page.route("**/api/wiki/**", async (r) => {
    const u = new URL(r.request().url());
    const path = u.pathname;
    const method = r.request().method();
    if (path === "/api/wiki/limits")
      return r.fulfill({
        json: {
          name_bytes: 512,
          markdown_bytes: 10000,
          search_bytes: 512,
          search_excerpt_characters: 200,
          page_size: 50,
          draft_save_delay_ms: 1000,
        },
      });
    if (path === "/api/wiki/namespaces")
      return r.fulfill({
        json: {
          items: [{ id: "ns", name: "Private", role: "owner" }],
          has_more: false,
        },
      });
    if (path === "/api/wiki/ns")
      return r.fulfill({ json: { id: "ns", name: "Private", role: "owner" } });
    if (path.endsWith("/links")) return r.fulfill({ json: [] });
    if (path.endsWith("/organization"))
      return r.fulfill({
        json: {
          parent: current.parent ? summary("parent", "Parent page") : null,
          children: { items: [], has_more: false },
          favorite: starred,
        },
      });
    if (path.endsWith("/favorite")) {
      writes++;
      await pending;
      starred = r.request().postDataJSON().favorite;
      return r.fulfill({ status: 204 });
    }
    if (path === "/api/wiki/ns/pages/p") return r.fulfill({ json: current });
    if (path === "/api/wiki/ns/pages" && method === "POST") {
      const b = r.request().postDataJSON();
      expect(b.expected_revision).toBe("r1");
      expect(b.change).toEqual({ action: "set_parent", parent: "parent" });
      current = { ...current, parent: b.change.parent, revision: "r2" };
      return r.fulfill({ json: current });
    }
    if (path === "/api/wiki/ns/pages")
      return r.fulfill({
        json: {
          items: [summary("p", "A page"), summary("parent", "Parent page")],
          has_more: false,
        },
      });
    if (path.endsWith("/collections/favorites"))
      return r.fulfill({
        json: {
          items: starred ? [summary("p", "A page")] : [],
          has_more: false,
        },
      });
    if (path.endsWith("/collections/standalone"))
      return r.fulfill({ json: { items: [], has_more: false } });
    return r.continue();
  });
  await page.goto("/wiki/ns/page/p");
  const star = page.getByRole("button", {
    name: "Add to favorites",
    exact: true,
  });
  await expect(star).toBeEnabled();
  const edit = page.getByRole("button", { name: "Edit", exact: true });
  const before = await edit.boundingBox();
  const starBox = await star.boundingBox();
  await star.dblclick();
  await expect(star).toHaveAttribute("aria-busy", "true");
  await expect(star).toBeDisabled();
  expect(await star.boundingBox()).toEqual(starBox);
  expect(await edit.boundingBox()).toEqual(before);
  release();
  const starredButton = page.getByRole("button", {
    name: "Remove from favorites",
    exact: true,
  });
  await expect(starredButton).toHaveAttribute("aria-pressed", "true");
  expect(writes).toBe(1);
  expect(await edit.boundingBox()).toEqual(before);
  await page
    .getByRole("button", { name: "Change parent", exact: true })
    .click();
  const modal = page.getByRole("dialog", { name: "Parent page", exact: true });
  await expect(modal.getByRole("searchbox")).toBeFocused();
  await modal.getByRole("button", { name: "Parent page", exact: true }).click();
  await expect(modal).toHaveCount(0);
  await expect(page.locator(".wiki-parent-label")).toContainText("Parent page");
  await expect(page.getByText("Original text", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "★ Favorites", exact: true }).click();
  await expect(page).toHaveURL(/\/favorites$/);
  await expect(
    page.getByRole("heading", { name: "Favorites", exact: true }),
  ).toBeVisible();
  await expect(page.locator(".wiki-collection-results")).toContainText(
    "A page",
  );
  await page
    .getByRole("button", { name: "Standalone pages", exact: true })
    .click();
  await expect(
    page.getByText("No pages in this collection.", { exact: true }),
  ).toBeVisible();
});
