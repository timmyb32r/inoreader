import { useEffect, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type { WikiHistory as History, WikiPage } from "../api/generated";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
export function WikiHistory({
  client,
  page,
  canEdit,
  pageSize,
  onRestored,
}: {
  client: WikiClient;
  page: WikiPage;
  canEdit: boolean;
  pageSize: number;
  onRestored: (p: WikiPage) => void;
}) {
  const [data, setData] = useState<History>({ items: [], has_more: false }),
    [offset, setOffset] = useState(0),
    [selected, setSelected] = useState<WikiPage | null>(null),
    [status, setStatus] = useState("Loading history…");
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Request failed");
  const load = async (o: number) => {
    const d = await client.history(page.namespace, page.id, o);
    setData(d);
    setOffset(o);
    setStatus("");
  };
  useEffect(() => {
    let active = true;
    client
      .history(page.namespace, page.id)
      .then((d) => {
        if (active) {
          setData(d);
          setStatus("");
        }
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, []);
  return (
    <section class="wiki-history">
      <h2>History</h2>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      <div class="wiki-history__grid">
        <div>
          {data.items.map((r) => (
            <AsyncButton
              onError={report}
              class="wiki-history__revision"
              aria-pressed={r.revision === selected?.revision}
              onPress={async () => {
                setSelected(
                  (await client.revision(page.namespace, page.id, r.revision))
                    .page,
                );
              }}
            >
              <time>{new Date(r.created_at).toLocaleString()}</time>
              <span>
                {r.author_name} · {r.action}
              </span>
            </AsyncButton>
          ))}
        </div>
        <div class="wiki-comparison">
          <div>
            <h3>Selected revision</h3>
            <pre>{selected?.markdown ?? "Select a revision"}</pre>
          </div>
          <div>
            <h3>Current revision</h3>
            <pre>{page.markdown}</pre>
          </div>
        </div>
      </div>
      <div class="wiki-pagination">
        <AsyncButton
          disabled={!offset}
          onError={report}
          onPress={() => load(offset - pageSize)}
        >
          Previous
        </AsyncButton>
        <AsyncButton
          disabled={!data.has_more}
          onError={report}
          onPress={() => load(offset + pageSize)}
        >
          Next
        </AsyncButton>
        <AsyncButton
          disabled={
            !canEdit ||
            !selected ||
            selected.revision === page.revision ||
            page.deleted
          }
          onError={report}
          onPress={async () => {
            if (
              !selected ||
              !confirm("Restore this revision as a new revision?")
            )
              return;
            onRestored(
              await client.write(page.namespace, {
                operation: crypto.randomUUID(),
                page: page.id,
                expected_revision: page.revision,
                change: {
                  action: "restore_revision",
                  revision: selected.revision,
                },
              }),
            );
          }}
        >
          Restore revision
        </AsyncButton>
      </div>
    </section>
  );
}
