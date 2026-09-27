import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

function files(root: string): string[] {
  return readdirSync(root).flatMap((name) => {
    const path = join(root, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

it("keeps native editable controls inside shared primitives", () => {
  const src = join(process.cwd(), "src");
  const offenders = files(src)
    .filter(
      (path) =>
        path.endsWith(".tsx") &&
        !path.endsWith("ui/fields.tsx") &&
        !path.endsWith(".test.tsx"),
    )
    .filter((path) =>
      /<(input|textarea|select)\b/.test(readFileSync(path, "utf8")),
    )
    .map((path) => relative(src, path));
  expect(offenders).toEqual([]);
});

it("restricts AuthField to authentication screens", () => {
  const src = join(process.cwd(), "src");
  const offenders = files(src)
    .filter(
      (path) =>
        path.endsWith(".tsx") &&
        !path.endsWith("ui/fields.tsx") &&
        !path.endsWith("AuthScreen.tsx") &&
        !path.endsWith(".test.tsx"),
    )
    .filter((path) => /\bAuthField\b/.test(readFileSync(path, "utf8")))
    .map((path) => relative(src, path));
  expect(offenders).toEqual([]);
});

it("keeps the subscriptions dialog on an opaque themed background", () => {
  const stylesheet = readFileSync(
    join(process.cwd(), "src/styles.css"),
    "utf8",
  );
  expect(stylesheet.replace(/\s+/g, "")).toMatch(
    /\.subscriptions-window\{[^}]*background:var\(--bg\)/,
  );
  expect(stylesheet).not.toContain("var(--app-bg)");
});

it("keeps feature modules independent from the App shell", () => {
  const src = join(process.cwd(), "src");
  const offenders = files(src)
    .filter(
      (path) =>
        /\.tsx?$/.test(path) &&
        !/\.test\.tsx?$/.test(path) &&
        !path.endsWith("main.tsx"),
    )
    .filter((path) =>
      /from\s+["'][^"']*\/App["']/.test(readFileSync(path, "utf8")),
    )
    .map((path) => relative(src, path));
  expect(offenders).toEqual([]);
});

it("resolves every CSS custom property to an authored token or explicit fallback", () => {
  const src = join(process.cwd(), "src");
  const text = files(src)
    .filter((p) => p.endsWith(".css"))
    .map((p) => readFileSync(p, "utf8"))
    .join("\n");
  const defined = new Set(
    [...text.matchAll(/(--[a-zA-Z0-9-]+)\s*:/g)].map((m) => m[1]),
  );
  const missing = [...text.matchAll(/var\((--[a-zA-Z0-9-]+)\s*([,)])/g)]
    .filter((m) => m[2] !== "," && !defined.has(m[1]))
    .map((m) => m[1]);
  expect([...new Set(missing)]).toEqual([]);
});
