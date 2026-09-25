import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

function files(root: string): string[] { return readdirSync(root).flatMap((name) => { const path = join(root, name); return statSync(path).isDirectory() ? files(path) : [path]; }); }

it("keeps native editable controls inside shared primitives", () => {
  const src = join(process.cwd(), "src");
  const offenders = files(src).filter((path) => path.endsWith(".tsx") && !path.endsWith("ui/fields.tsx") && !path.endsWith(".test.tsx")).filter((path) => /<(input|textarea|select)\b/.test(readFileSync(path, "utf8"))).map((path) => relative(src, path));
  expect(offenders).toEqual([]);
});

it("restricts AuthField to authentication screens", () => {
  const src = join(process.cwd(), "src");
  const offenders = files(src).filter((path) => path.endsWith(".tsx") && !path.endsWith("ui/fields.tsx") && !path.endsWith("AuthScreen.tsx") && !path.endsWith(".test.tsx")).filter((path) => /\bAuthField\b/.test(readFileSync(path, "utf8"))).map((path) => relative(src, path));
  expect(offenders).toEqual([]);
});
