import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type {
  WikiPage,
  WikiOrganization as Organization,
} from "../api/generated";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import { WikiParentPicker } from "./WikiParentPicker";
export function WikiOrganization({
  client,
  page,
  canEdit,
  pageSize,
  navigate,
  onSaved,
}: {
  client: WikiClient;
  page: WikiPage;
  canEdit: boolean;
  pageSize: number;
  navigate: (p: string) => boolean;
  onSaved: (p: WikiPage) => void;
}) {
  const [data, setData] = useState<Organization | null>(null),
    [loading, setLoading] = useState(true),
    [offset, setOffset] = useState(0),
    [picker, setPicker] = useState(false),
    [status, setStatus] = useState("");
  const generation = useRef(0);
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  const load = async (o: number) => {
    const g = ++generation.current;
    setLoading(true);
    try {
      const v = await client.organization(page.namespace, page.id, o);
      if (g === generation.current) {
        setData(v);
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
  }, [page.revision]);
  const go = (id: string) => navigate(`/wiki/${page.namespace}/page/${id}`);
  return (
    <section
      class="wiki-organization"
      aria-label="Page organization"
      aria-busy={loading}
    >
      <div class="wiki-organization-controls">
        <AsyncButton
          class="wiki-star"
          aria-label={
            data?.favorite ? "Remove from favorites" : "Add to favorites"
          }
          aria-pressed={data?.favorite ?? false}
          disabled={!data || page.deleted}
          onError={report}
          onPress={async () => {
            if (!data) return;
            const next = !data.favorite;
            await client.favorite(page.namespace, page.id, next);
            setData((current) =>
              current ? { ...current, favorite: next } : current,
            );
          }}
        >
          {data?.favorite ? "★" : "☆"}
        </AsyncButton>
        <span class="wiki-parent-label">
          Parent:{" "}
          {data?.parent ? (
            <button onClick={() => go(data.parent!.id)}>
              {data.parent.name}
            </button>
          ) : !data ? (
            loading ? (
              "Loading…"
            ) : (
              "Unavailable"
            )
          ) : page.parent ? (
            "Page in trash"
          ) : (
            "None"
          )}
        </span>
        <button
          disabled={!data || !canEdit || page.deleted}
          onClick={() => setPicker(true)}
        >
          Change parent
        </button>
      </div>
      <div class="wiki-children" aria-label="Child pages">
        {!data ? (
          loading ? (
            <span class="spinner" />
          ) : (
            <span class="wiki-muted">Hierarchy unavailable</span>
          )
        ) : data.children.items.length ? (
          data.children.items.map((p) => (
            <button onClick={() => go(p.id)}>{p.name}</button>
          ))
        ) : (
          <span class="wiki-muted">No child pages</span>
        )}
      </div>
      <div class="wiki-organization-footer">
        <StatusRegion class="wiki-status">{status}</StatusRegion>
        <AsyncButton
          disabled={!data || !offset}
          onError={report}
          onPress={() => load(offset - pageSize)}
        >
          Previous children
        </AsyncButton>
        <AsyncButton
          disabled={!data?.children.has_more}
          onError={report}
          onPress={() => load(offset + pageSize)}
        >
          Next children
        </AsyncButton>
      </div>
      {picker && (
        <WikiParentPicker
          {...{ client, page, pageSize, onSaved }}
          onClose={() => setPicker(false)}
        />
      )}
    </section>
  );
}
