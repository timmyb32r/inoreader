import { render, screen } from "@testing-library/preact";
import { ArticleReader } from "./ArticleReader";
import type { Article } from "../api/viewModels";

const article: Article = {
  id: "a",
  url: "https://example.test",
  source: "Source",
  title: "Article",
  excerpt: "",
  body: [],
  savedAt: "2026-10-06T00:00:00Z",
  read: false,
  later: false,
  fullText: "pending",
};
const noop = () => {};
const props = {
  pending: new Set<string>(),
  className: "",
  onBack: noop,
  onOpenSubscription: noop,
  onUpdate: noop,
  onRefresh: async () => {},
  onNotice: noop,
  onSummarize: noop,
  summaryPending: false,
  onDefinitions: noop,
  definitionsPending: false,
  onNavigate: noop,
  workspaceId: "ws",
  aiClient: {} as never,
  wikiClient: {} as never,
  translationEnabled: false,
};
describe("article read provenance", () => {
  it("distinguishes reader, bulk, individual and unknown without moving the metadata slot", () => {
    const view = render(<ArticleReader {...props} article={article} />);
    const slot = screen.getByLabelText("Способ чтения");
    expect(slot).toHaveTextContent("Не прочитано");
    for (const [readMethod, label] of [
      ["reader", "Прочитано в ридере"],
      ["bulk", "Отмечено через Mark all read"],
      ["single", "Отмечено отдельно"],
      ["unknown", "способ неизвестен"],
    ] as const) {
      view.rerender(
        <ArticleReader
          {...props}
          article={{ ...article, read: true, readMethod }}
        />,
      );
      expect(screen.getByLabelText("Способ чтения")).toBe(slot);
      expect(slot).toHaveTextContent(label);
    }
    view.rerender(
      <ArticleReader
        {...props}
        article={{ ...article, read: false, readMethod: "bulk" }}
      />,
    );
    expect(slot).toHaveTextContent("Не прочитано");
  });
});
