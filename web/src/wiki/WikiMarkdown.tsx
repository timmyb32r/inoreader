import { AsyncButton } from "../ui/AsyncButton";
import { h, type ComponentChildren } from "preact";
/** Render only text nodes and explicitly allowed elements. Raw HTML and images
 * stay literal. Wiki navigation is owned by the current namespace callback. */
export function WikiMarkdown({
  text,
  onLink,
  links,
}: {
  text: string;
  links?: Record<string, boolean>;
  onLink: (name: string) => void | Promise<void>;
}) {
  const inline = (s: string): ComponentChildren[] => {
    const result: ComponentChildren[] = [];
    const tokens =
      /(`[^`\n]+`|\[\[[^\[\]\n]+\]\]|\*\*[^*]+\*\*|__[^_]+__|\*[^*\n]+\*|_[^_\n]+_|!?\[[^\]\n]+\]\([^\s)]+\))/g;
    let at = 0;
    for (const m of s.matchAll(tokens)) {
      result.push(s.slice(at, m.index));
      const t = m[0];
      if (t.startsWith("`")) result.push(<code>{t.slice(1, -1)}</code>);
      else if (t.startsWith("[["))
        result.push(
          <AsyncButton
            class={`wiki-link${links?.[t.slice(2, -2)] === false ? " wiki-link--missing" : ""}`}
            onPress={async () => {
              await onLink(t.slice(2, -2));
            }}
            onError={() => {}}
          >
            {t.slice(2, -2)}
          </AsyncButton>,
        );
      else if (t.startsWith("**") || t.startsWith("__"))
        result.push(<strong>{t.slice(2, -2)}</strong>);
      else if (t.startsWith("*") || t.startsWith("_"))
        result.push(<em>{t.slice(1, -1)}</em>);
      else {
        const m = /^(!?)\[([^\]]+)\]\(([^)]+)\)$/.exec(t)!;
        let safe = false;
        try {
          const u = new URL(m[3], location.href);
          safe =
            /^https?:$/.test(u.protocol) &&
            !m[1] &&
            !(
              u.origin === location.origin &&
              decodeURIComponent(u.pathname).startsWith("/wiki")
            );
        } catch {
          /* literal invalid URL */
        }
        result.push(
          safe ? (
            <a
              href={m[3]}
              target="_blank"
              rel="noopener noreferrer"
              referrerPolicy="no-referrer"
            >
              {m[2]}
            </a>
          ) : (
            t
          ),
        );
      }
      at = m.index + t.length;
    }
    result.push(s.slice(at));
    return result;
  };
  const lines = text.split("\n"),
    blocks: ComponentChildren[] = [];
  for (let i = 0; i < lines.length; ) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    if (line.startsWith("```")) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```"))
        code.push(lines[i++]);
      if (i < lines.length) i++;
      blocks.push(
        <pre>
          <code>{code.join("\n")}</code>
        </pre>,
      );
      continue;
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line);
    if (heading) {
      blocks.push(h(`h${heading[1].length}`, {}, inline(heading[2])));
      i++;
      continue;
    }
    if (/^> ?/.test(line)) {
      const quote: string[] = [];
      while (i < lines.length && /^> ?/.test(lines[i]))
        quote.push(lines[i++].replace(/^> ?/, ""));
      blocks.push(<blockquote>{inline(quote.join("\n"))}</blockquote>);
      continue;
    }
    if (/^\s*([-*+] |\d+\. )/.test(line)) {
      const ordered = /^\s*\d+\. /.test(line),
        items: ComponentChildren[] = [];
      const pattern = ordered ? /^\s*\d+\. (.*)$/ : /^\s*[-*+] (.*)$/;
      while (i < lines.length) {
        const match = pattern.exec(lines[i]);
        if (!match) break;
        items.push(<li>{inline(match[1])}</li>);
        i++;
      }
      blocks.push(
        ordered ? (
          <ol start={parseInt(line, 10)}>{items}</ol>
        ) : (
          <ul>{items}</ul>
        ),
      );
      continue;
    }
    if (
      i + 1 < lines.length &&
      line.includes("|") &&
      /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*$/.test(lines[i + 1])
    ) {
      const cells = (s: string) =>
        s
          .replace(/^\s*\|/, "")
          .replace(/\|\s*$/, "")
          .split("|");
      const headers = cells(line);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && lines[i].includes("|"))
        rows.push(cells(lines[i++]));
      blocks.push(
        <div class="wiki-table">
          <table>
            <thead>
              <tr>
                {headers.map((c) => (
                  <th>{inline(c)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr>
                  {row.map((c) => (
                    <td>{inline(c)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    const paragraph = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(```|> ?|#{1,6}\s|\s*[-*+] |\s*\d+\. )/.test(lines[i])
    )
      paragraph.push(lines[i++]);
    blocks.push(<p>{inline(paragraph.join("\n"))}</p>);
  }
  return <div class="wiki-markdown">{blocks}</div>;
}
