import type { ComponentChildren } from "preact";
import { CopyButton } from "../ui/CopyButton";

/** Small, deliberately non-HTML Markdown renderer. Provider strings always become
 * text nodes; links accept only HTTP(S), and images never trigger remote loads. */
export function ChatMarkdown({ text }: { text: string }) {
  const lines = text.split("\n"),
    blocks: ComponentChildren[] = [];
  for (let index = 0; index < lines.length; ) {
    const line = lines[index];
    if (!line.trim()) {
      index += 1;
      continue;
    }
    if (/^```/.test(line)) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !/^```/.test(lines[index]))
        code.push(lines[index++]);
      if (index < lines.length) index += 1;
      blocks.push(
        <pre key={index}>
          <code>{code.join("\n")}</code>
        </pre>,
      );
      continue;
    }
    if (/^> ?/.test(line)) {
      const quoted: string[] = [];
      while (index < lines.length && /^> ?/.test(lines[index]))
        quoted.push(lines[index++].replace(/^> ?/, ""));
      const quote = quoted.join("\n");
      blocks.push(
        <blockquote key={index}>
          <p>{inline(quote)}</p>
          <CopyButton text={quote} label="Copy quote" />
        </blockquote>,
      );
      continue;
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line);
    if (heading) {
      blocks.push(
        <p class="ai-markdown__heading" key={index}>
          {inline(heading[2])}
        </p>,
      );
      index += 1;
      continue;
    }
    if (/^\s*([-*+] |\d+\. )/.test(line)) {
      const ordered = /^\s*\d+\. /.test(line),
        items: ComponentChildren[] = [];
      const matcher = ordered ? /^\s*\d+\. (.*)$/ : /^\s*[-*+] (.*)$/;
      const start = ordered ? Number.parseInt(line.trim(), 10) : undefined;
      while (index < lines.length) {
        const match = matcher.exec(lines[index]);
        if (!match) break;
        items.push(<li key={index}>{inline(match[1])}</li>);
        index += 1;
      }
      blocks.push(
        ordered ? (
          <ol key={index} start={start}>
            {items}
          </ol>
        ) : (
          <ul key={index}>{items}</ul>
        ),
      );
      continue;
    }
    const paragraph: string[] = [line];
    index += 1;
    while (
      index < lines.length &&
      lines[index].trim() &&
      !/^(```|> ?|#{1,6}\s|\s*[-*+] |\s*\d+\. )/.test(lines[index])
    )
      paragraph.push(lines[index++]);
    blocks.push(<p key={index}>{inline(paragraph.join("\n"))}</p>);
  }
  return <div class="ai-markdown">{blocks}</div>;
}

function inline(text: string): ComponentChildren[] {
  const result: ComponentChildren[] = [];
  const tokens =
    /(`[^`\n]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*\n]+\*|_[^_\n]+_|!?\[[^\]\n]+\]\([^\s)]+\))/g;
  let previous = 0;
  for (const match of text.matchAll(tokens)) {
    result.push(text.slice(previous, match.index));
    const token = match[0];
    if (token.startsWith("`"))
      result.push(<code key={match.index}>{token.slice(1, -1)}</code>);
    else if (token.startsWith("**") || token.startsWith("__"))
      result.push(<strong key={match.index}>{token.slice(2, -2)}</strong>);
    else if (token.startsWith("*") || token.startsWith("_"))
      result.push(<em key={match.index}>{token.slice(1, -1)}</em>);
    else {
      const link = /^(!?)\[([^\]]+)\]\(([^)]+)\)$/.exec(token)!;
      const href = safeLink(link[3]);
      result.push(
        href && !link[1] ? (
          <a
            key={match.index}
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            referrerPolicy="no-referrer"
          >
            {link[2]}
          </a>
        ) : (
          token
        ),
      );
    }
    previous = match.index + token.length;
  }
  result.push(text.slice(previous));
  return result;
}

function safeLink(value: string): string | undefined {
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) ? value : undefined;
  } catch {
    return undefined;
  }
}
