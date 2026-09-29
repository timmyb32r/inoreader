import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type { WikiPage, WikiPages } from "../api/generated";
import { ModalDialog } from "../ui/ModalDialog";
import { SearchField } from "../ui/SearchField";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
export function WikiParentPicker({
  client,
  page,
  pageSize,
  onSaved,
  onClose,
}: {
  client: WikiClient;
  page: WikiPage;
  pageSize: number;
  onSaved: (p: WikiPage) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState(""),
    [result, setResult] = useState<WikiPages>({ items: [], has_more: false }),
    [offset, setOffset] = useState(0),
    [loading, setLoading] = useState(true),
    [saving, setSaving] = useState(false),
    [status, setStatus] = useState("");
  const generation = useRef(0);
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Could not change parent");
  const load = async (o: number) => {
    const g = ++generation.current;
    setLoading(true);
    try {
      const data = await client.pages(page.namespace, query, false, o);
      if (g === generation.current) {
        setResult(data);
        setOffset(o);
      }
    } finally {
      if (g === generation.current) setLoading(false);
    }
  };
  useEffect(() => {
    void load(0).catch(report);
    return () => {
      generation.current++;
    };
  }, []);
  const choose = async (parent: string | null) => {
    setSaving(true);
    setStatus("");
    try {
      const p = await client.write(page.namespace, {
        operation: crypto.randomUUID(),
        page: page.id,
        expected_revision: page.revision,
        change: { action: "set_parent", parent },
      });
      onSaved(p);
      onClose();
    } finally {
      setSaving(false);
    }
  };
  return (
    <ModalDialog
      title="Parent page"
      onClose={() => {
        if (!saving) onClose();
      }}
    >
      <div class="wiki-parent-picker">
        <SearchField
          autoFocus
          value={query}
          onInput={setQuery}
          onSubmit={() => void load(0).catch(report)}
          busy={loading || saving}
          label="Find parent page"
        />
        <StatusRegion class="wiki-status">{status}</StatusRegion>
        <AsyncButton
          disabled={saving || !page.parent}
          onError={report}
          onPress={() => choose(null)}
        >
          No parent
        </AsyncButton>
        <div class="wiki-parent-results" aria-busy={loading}>
          {loading ? (
            <span class="spinner" />
          ) : (
            result.items
              .filter((p) => p.id !== page.id)
              .map((p) => (
                <AsyncButton
                  disabled={saving || p.id === page.parent}
                  onError={report}
                  onPress={() => choose(p.id)}
                >
                  {p.name}
                </AsyncButton>
              ))
          )}
          {!loading && !result.items.filter((p) => p.id !== page.id).length && (
            <p>No matching pages</p>
          )}
        </div>
        <div class="wiki-pagination">
          <AsyncButton
            disabled={saving || loading || !offset}
            onError={report}
            onPress={() => load(offset - pageSize)}
          >
            Previous
          </AsyncButton>
          <AsyncButton
            disabled={saving || loading || !result.has_more}
            onError={report}
            onPress={() => load(offset + pageSize)}
          >
            Next
          </AsyncButton>
        </div>
      </div>
    </ModalDialog>
  );
}
