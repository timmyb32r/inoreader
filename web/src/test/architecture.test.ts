import ts from "typescript";
import { imports, sourceGraph, violations } from "../../tools/architecture";
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

it("enforces resolved feature boundaries and runtime import acyclicity", () => {
  const root = join(process.cwd(), "src");
  const config = ts.readConfigFile(
    join(process.cwd(), "tsconfig.json"),
    ts.sys.readFile,
  );
  const options = ts.parseJsonConfigFileContent(
    config.config,
    ts.sys,
    process.cwd(),
  ).options;
  const production = files(root).filter(
    (path) =>
      /\.tsx?$/.test(path) &&
      !/\.test\.tsx?$/.test(path) &&
      !path.includes("/test/") &&
      !path.includes("/tests/"),
  );
  expect(violations(sourceGraph(production, root, options))).toEqual([]);
});
it("detects renamed, re-exported and dynamic imports and cycles", () => {
  expect(
    imports(
      'import type { X as Y } from "../app/ReaderApplication"; export { z } from "./z"; import("./lazy");',
    ),
  ).toEqual([
    { target: "../app/ReaderApplication", runtime: false },
    { target: "./z", runtime: true },
    { target: "./lazy", runtime: true },
  ]);
  expect(
    violations(
      new Map([
        [
          "api/example.ts",
          [{ target: "app/ReaderApplication.tsx", runtime: false }],
        ],
        ["ai/a.ts", [{ target: "ai/b.ts", runtime: true }]],
        ["ai/b.ts", [{ target: "ai/a.ts", runtime: true }]],
      ]),
    ),
  ).toEqual(
    expect.arrayContaining([
      expect.stringContaining("api/example.ts must not import"),
      expect.stringContaining("Runtime import cycle"),
    ]),
  );
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
