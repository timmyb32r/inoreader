import ts from "typescript";
import { relative, resolve, sep } from "node:path";

export type Edge = { target: string; runtime: boolean };
export function imports(text: string): Edge[] {
  const file = ts.createSourceFile(
    "module.tsx",
    text,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  const edges: Edge[] = [];
  const add = (node: ts.Node | undefined, runtime: boolean) => {
    if (node && ts.isStringLiteral(node))
      edges.push({ target: node.text, runtime });
  };
  const visit = (node: ts.Node) => {
    if (ts.isImportDeclaration(node)) {
      const clause = node.importClause;
      const named = clause?.namedBindings;
      const typeOnly =
        clause?.isTypeOnly ||
        (!clause?.name &&
          named &&
          ts.isNamedImports(named) &&
          named.elements.length > 0 &&
          named.elements.every((v) => v.isTypeOnly));
      add(node.moduleSpecifier, !typeOnly);
    } else if (ts.isExportDeclaration(node)) {
      add(node.moduleSpecifier, !node.isTypeOnly);
    } else if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) &&
          node.expression.text === "require"))
    ) {
      add(node.arguments[0], true);
    } else if (
      ts.isImportEqualsDeclaration(node) &&
      ts.isExternalModuleReference(node.moduleReference)
    ) {
      add(node.moduleReference.expression, !node.isTypeOnly);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return edges;
}

const layers = new Set(["app", "api", "ui", "ai", "translation", "glossary"]);
const layer = (path: string) =>
  layers.has(path.split("/")[0]) ? path.split("/")[0] : "shared";
const allowed: Record<string, string[]> = {
  app: [...layers, "shared"],
  api: ["api", "shared"],
  ui: ["ui", "shared"],
  ai: ["ai", "api", "ui", "shared"],
  translation: ["translation", "api", "ui", "shared"],
  glossary: ["glossary", "api", "ui", "shared"],
  shared: ["shared"],
};

export function violations(graph: Map<string, Edge[]>): string[] {
  const errors: string[] = [];
  for (const [source, edges] of graph) {
    for (const { target } of edges) {
      if (source === "main.tsx") continue;
      if (!allowed[layer(source)].includes(layer(target)))
        errors.push(`${source} must not import ${target}`);
      if (target === "app/ReaderApplication.tsx" && source !== "app/App.tsx")
        errors.push(`${source} imports application composition`);
      if (target === "app/App.tsx")
        errors.push(`${source} imports root composition`);
    }
  }
  const visited = new Set<string>();
  const visiting: string[] = [];
  const visit = (source: string) => {
    if (visiting.includes(source)) {
      errors.push(
        `Runtime import cycle: ${[...visiting, source].join(" -> ")}`,
      );
      return;
    }
    if (visited.has(source)) return;
    visiting.push(source);
    for (const edge of graph.get(source) ?? [])
      if (edge.runtime) visit(edge.target);
    visiting.pop();
    visited.add(source);
  };
  for (const source of graph.keys()) visit(source);
  return errors;
}

export function sourceGraph(
  files: string[],
  root: string,
  options: ts.CompilerOptions,
): Map<string, Edge[]> {
  const graph = new Map<string, Edge[]>();
  const key = (file: string) => relative(root, file).split(sep).join("/");
  for (const file of files) {
    const edges = imports(ts.sys.readFile(file) ?? "").flatMap((edge) => {
      const found = ts.resolveModuleName(
        edge.target,
        file,
        options,
        ts.sys,
      ).resolvedModule;
      if (
        !found ||
        !resolve(found.resolvedFileName).startsWith(resolve(root) + sep)
      )
        return [];
      return [{ target: key(found.resolvedFileName), runtime: edge.runtime }];
    });
    graph.set(key(file), edges);
  }
  return graph;
}
