/** Highlight literal matches as text nodes; query and excerpts never become HTML. */
export function MatchText({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const matches = Array.from(text.matchAll(new RegExp(escaped, "giu")));
  let offset = 0;
  const parts = [];
  for (const match of matches) {
    parts.push(text.slice(offset, match.index));
    parts.push(<mark>{match[0]}</mark>);
    offset = match.index + match[0].length;
  }
  parts.push(text.slice(offset));
  return <>{parts}</>;
}
