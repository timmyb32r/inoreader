import { describe, expect, it } from "vitest";
import { commitTldr } from "./CommitGroup";
import { commitProject } from "./commitArticle";
describe("project commits", () => {
  it("extracts the complete TL;DR paragraph instead of the commit introduction", () => {
    expect(
      commitTldr(
        "# Title\n\nКоммит в репозитории engine.\n\n**TL;DR:** Перенесли обработку на отдельный поток. Медленный диск больше не останавливает воркер.\n\n## Details\n\nOther text",
      ),
    ).toBe(
      "Перенесли обработку на отдельный поток. Медленный диск больше не останавливает воркер.",
    );
    expect(commitTldr("## TL;DR\n\nПолный абзац.\n\nПодробности")).toBe(
      "Полный абзац.",
    );
    expect(commitTldr("# Title\n\nСодержательный абзац без маркировки.")).toBe(
      "Содержательный абзац без маркировки.",
    );
  });
  it("groups by authored repository path, never similar source names", () => {
    expect(commitProject("https://github.com/acme/engine/commit/abc")).toBe(
      "acme/engine",
    );
    expect(
      commitProject("https://github.com/acme/engine-extra/commits/def"),
    ).toBe("acme/engine-extra");
    expect(
      commitProject("https://github.com.evil/acme/engine/commit/abc"),
    ).toBeNull();
    expect(commitProject("https://github.com/acme/engine/issues/1")).toBeNull();
  });
});
