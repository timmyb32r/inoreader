import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Minimized publisher DOM fixtures retain all original article URLs and presentation
// duplicates, while replacing prose and stripping scripts/assets. These explicit
// publisher recipes select article presentations; ingestion identity checks remain strict.
const fixtures = JSON.parse(
  readFileSync(
    "../crates/reader-ingest/src/tests/fixtures/subscription_selectors.json",
    "utf8",
  ),
);

describe("subscription article selectors", () => {
  for (const fixture of fixtures) {
    it(`${fixture.name}: retains every expected article exactly once`, () => {
      const document = new DOMParser().parseFromString(
        fixture.html,
        "text/html",
      );
      const recipe = fixture.kind.WebPage;
      let selected: Element[];
      if (recipe.selector.language === "x_path") {
        const result = document.evaluate(
          recipe.selector.expression,
          document,
          null,
          XPathResult.ORDERED_NODE_SNAPSHOT_TYPE,
          null,
        );
        selected = Array.from(
          { length: result.snapshotLength },
          (_, index) => result.snapshotItem(index) as Element,
        );
      } else if (recipe.extraction.card_selector) {
        selected = Array.from(
          document.querySelectorAll(recipe.extraction.card_selector.expression),
        )
          .map((card) => card.querySelector(recipe.selector.expression))
          .filter((node): node is Element => node !== null);
      } else {
        selected = Array.from(
          document.querySelectorAll(recipe.selector.expression),
        );
      }
      const urls = selected
        .map((node) => {
          const link = node.matches("a[href]")
            ? node
            : node.querySelector("a[href]");
          expect(link).not.toBeNull();
          return new URL(link!.getAttribute("href")!, fixture.baseUrl).href;
        })
        .filter((url) => new RegExp(recipe.extraction.url_pattern).test(url));
      expect(new Set(urls).size).toBe(urls.length);
      expect(urls.sort()).toEqual(fixture.expectedUrls);
    });
  }
});
