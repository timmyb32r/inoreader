import { WikiOrganization } from "./WikiOrganization";
import { SearchField } from "../ui/SearchField";
import "../search/search.css";
import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import { ApiError } from "../api/client";
import type {
  WikiLimits,
  WikiNamespace,
  WikiPage,
  WikiPages,
} from "../api/generated";
import { AutofillResistantField as Field } from "../ui/fields";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import { ModalDialog } from "../ui/ModalDialog";
import { WikiMarkdown } from "./WikiMarkdown";
import { WikiEditor } from "./WikiEditor";
import { WikiHistory } from "./WikiHistory";
import { WikiAccess } from "./WikiAccess";
export function NamespaceWorkspace({
  client,
  namespace,
  parts,
  limits,
  navigate,
  onDirty,
  onBack,
}: {
  client: WikiClient;
  namespace: string;
  parts: string[];
  limits: WikiLimits;
  navigate: (p: string) => boolean;
  onDirty: (d: boolean) => void;
  onBack: () => void;
}) {
  const [ns, setNs] = useState<WikiNamespace | null>(null),
    [list, setList] = useState<WikiPages>({ items: [], has_more: false }),
    [search, setSearch] = useState(""),
    [offset, setOffset] = useState(0),
    [status, setStatus] = useState("Loading namespace…"),
    [page, setPage] = useState<WikiPage | null>(null),
    [editing, setEditing] = useState(false),
    [history, setHistory] = useState(false),
    [missing, setMissing] = useState<string | null>(null),
    [renaming, setRenaming] = useState(false),
    [newName, setNewName] = useState("");
  const [links, setLinks] = useState<Record<string, boolean>>({});
  const generation = useRef(0);
  const listGeneration = useRef(0);
  const mode = parts[2] ?? "",
    id = parts[3];
  const collection =
    mode === "favorites" || mode === "standalone" ? mode : null;
  const trash = mode === "trash",
    access = mode === "access",
    isNew = mode === "new";
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  const canEdit = !!ns && ns.role !== "reader";
  const loadList = async (o = offset, s = search) => {
    const token = generation.current,
      sequence = ++listGeneration.current;
    const data = await (collection
      ? client.collection(namespace, collection, o)
      : client.pages(namespace, s, trash, o));
    if (token !== generation.current || sequence !== listGeneration.current)
      return;
    setList(data);
    setOffset(o);
  };
  useEffect(() => {
    let active = true;
    client
      .namespace(namespace)
      .then((n) => {
        if (active) setNs(n);
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, []);
  useEffect(() => {
    const token = ++generation.current;
    setPage(null);
    setLinks({});
    setEditing(isNew);
    setHistory(false);
    setMissing(null);
    setRenaming(false);
    setStatus("Loading…");
    Promise.all([
      collection
        ? client.collection(namespace, collection)
        : client.pages(namespace, "", trash),
      mode === "page" && id
        ? client.page(namespace, id)
        : Promise.resolve(null),
    ])
      .then(([list, p]) => {
        if (token !== generation.current) return;
        setList(list);
        setPage(p);
        setOffset(0);
        setStatus("");
      })
      .catch((e) => {
        if (token === generation.current) report(e);
      });
    return () => {
      generation.current++;
    };
  }, [mode, id]);
  useEffect(() => {
    if (!page) return;
    let active = true;
    client
      .links(namespace, page.id)
      .then((items) => {
        if (active)
          setLinks(Object.fromEntries(items.map((l) => [l.name, !!l.page])));
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, [page?.revision]);
  const open = (p: WikiPage) => {
    onDirty(false);
    setEditing(false);
    setHistory(false);
    if (parts[3] === p.id) {
      setPage(p);
      void loadList();
    } else navigate(`/wiki/${namespace}/page/${p.id}`);
  };
  const link = async (name: string) => {
    const token = generation.current;
    setStatus("Opening page…");
    try {
      const p = await client.resolve(namespace, name);
      if (token !== generation.current) return;
      if (p.deleted) {
        setStatus("This page is in trash.");
        return;
      }
      setStatus("");
      navigate(`/wiki/${namespace}/page/${p.id}`);
    } catch (e) {
      if (token !== generation.current) return;
      if (e instanceof ApiError && e.status === 404) {
        setMissing(name);
        setStatus("Page does not exist. It will be created only after saving.");
      } else report(e);
    }
  };
  const cancel = () => {
    if (confirm("Close editor? Saved private drafts remain available.")) {
      onDirty(false);
      setEditing(false);
      if (isNew) navigate(`/wiki/${namespace}`);
    }
  };
  return (
    <section class="wiki-shell wiki-workspace">
      <aside class="wiki-nav">
        <button onClick={onBack}>← Back to Reader</button>
        <button onClick={() => navigate("/wiki")}>← Namespaces</button>
        <h2>{ns?.name ?? "Wiki"}</h2>
        <div class="wiki-search">
          {trash ? (
            <>
              <Field
                aria-label="Filter deleted pages"
                value={search}
                onInput={(e) => setSearch(e.currentTarget.value)}
              />
              <AsyncButton
                onError={report}
                onPress={async () => {
                  if (
                    new TextEncoder().encode(search).length >
                    limits.search_bytes
                  )
                    throw new Error("Search exceeds configured byte limit");
                  await loadList(0);
                }}
              >
                Search
              </AsyncButton>
            </>
          ) : (
            <SearchField
              value={search}
              onInput={setSearch}
              label="Search this wiki"
              onSubmit={() =>
                navigate(
                  `/search?${new URLSearchParams({ q: search, kind: "wiki", namespace })}`,
                )
              }
            />
          )}
        </div>
        <button
          disabled={!canEdit}
          onClick={() =>
            navigate(`/wiki/${namespace}/new/${crypto.randomUUID()}`)
          }
        >
          New page
        </button>
        <nav class="wiki-collections" aria-label="Wiki collections">
          <button
            class={!collection && !trash ? "active" : ""}
            onClick={() => navigate(`/wiki/${namespace}`)}
          >
            All pages
          </button>
          <button
            class={collection === "favorites" ? "active" : ""}
            onClick={() => navigate(`/wiki/${namespace}/favorites`)}
          >
            ★ Favorites
          </button>
          <button
            class={collection === "standalone" ? "active" : ""}
            onClick={() => navigate(`/wiki/${namespace}/standalone`)}
          >
            Standalone pages
          </button>
        </nav>
        <div class="wiki-page-list">
          {!collection &&
            list.items.map((p) => (
              <button
                class={p.id === id ? "active" : ""}
                onClick={() => navigate(`/wiki/${namespace}/page/${p.id}`)}
              >
                {p.name}
                {p.excerpt && (
                  <small class="wiki-search-excerpt">
                    {p.excerpt}
                    {p.excerpt_truncated ? "…" : ""}
                  </small>
                )}
              </button>
            ))}
        </div>
        <div class="wiki-pagination">
          <AsyncButton
            disabled={!offset}
            onError={report}
            onPress={() => loadList(offset - limits.page_size)}
          >
            Previous
          </AsyncButton>
          <AsyncButton
            disabled={!list.has_more}
            onError={report}
            onPress={() => loadList(offset + limits.page_size)}
          >
            Next
          </AsyncButton>
        </div>
        <button
          disabled={!canEdit}
          onClick={() => navigate(`/wiki/${namespace}/trash`)}
        >
          Trash
        </button>
        <button
          disabled={ns?.role !== "owner"}
          onClick={() => navigate(`/wiki/${namespace}/access`)}
        >
          Access
        </button>
      </aside>
      <main class="wiki-content">
        <StatusRegion class="wiki-status">{status}</StatusRegion>
        {access && ns?.role === "owner" ? (
          <WikiAccess
            key={namespace}
            client={client}
            namespace={namespace}
            pageSize={limits.page_size}
          />
        ) : editing && canEdit && id ? (
          <WikiEditor
            key={id}
            {...{ client, namespace, id, page, limits, onDirty }}
            initialName={parts[4] ? decodeURIComponent(parts[4]) : ""}
            onSaved={open}
            onCancel={cancel}
          />
        ) : page ? (
          <>
            <header class="wiki-page-header">
              <h1>{page.name}</h1>
              <div class="wiki-actions">
                <button
                  disabled={!canEdit || page.deleted}
                  onClick={() => setEditing(true)}
                >
                  Edit
                </button>
                <details>
                  <summary aria-label="Page actions">⋯</summary>
                  <div>
                    <button onClick={() => setHistory(!history)}>
                      History
                    </button>
                    <button
                      disabled={!canEdit || page.deleted}
                      onClick={() => {
                        setNewName(page.name);
                        setRenaming(true);
                      }}
                    >
                      Rename
                    </button>
                    <AsyncButton
                      disabled={!canEdit}
                      onError={report}
                      onPress={async () => {
                        if (
                          !confirm(
                            page.deleted
                              ? "Restore this page?"
                              : "Move this page to trash?",
                          )
                        )
                          return;
                        open(
                          await client.write(namespace, {
                            operation: crypto.randomUUID(),
                            page: page.id,
                            expected_revision: page.revision,
                            change: {
                              action: page.deleted ? "restore" : "trash",
                            },
                          }),
                        );
                      }}
                    >
                      {page.deleted ? "Restore" : "Move to trash"}
                    </AsyncButton>
                  </div>
                </details>
              </div>
            </header>
            <WikiOrganization
              key={page.id}
              {...{ client, page, canEdit, navigate }}
              pageSize={limits.page_size}
              onSaved={open}
            />
            {page.deleted && (
              <p>This page is in trash. Its history is preserved.</p>
            )}
            {renaming && (
              <ModalDialog
                title="Rename page"
                onClose={() => setRenaming(false)}
              >
                <div class="wiki-rename">
                  <Field
                    aria-label="New page name"
                    value={newName}
                    onInput={(e) => setNewName(e.currentTarget.value)}
                  />
                  <AsyncButton
                    onError={report}
                    onPress={async () => {
                      if (
                        !newName ||
                        /[\n\r\[\]\0]/.test(newName) ||
                        new TextEncoder().encode(newName).length >
                          limits.name_bytes
                      )
                        throw new Error("Invalid page name");
                      open(
                        await client.write(namespace, {
                          operation: crypto.randomUUID(),
                          page: page.id,
                          expected_revision: page.revision,
                          change: { action: "rename", name: newName },
                        }),
                      );
                      setRenaming(false);
                    }}
                  >
                    Rename
                  </AsyncButton>
                </div>
              </ModalDialog>
            )}
            {history ? (
              <WikiHistory
                key={page.revision}
                client={client}
                page={page}
                canEdit={canEdit}
                pageSize={limits.page_size}
                onRestored={open}
              />
            ) : (
              <WikiMarkdown text={page.markdown} onLink={link} links={links} />
            )}
          </>
        ) : (
          <div class="wiki-empty">
            <h1>
              {trash
                ? "Trash"
                : collection === "favorites"
                  ? "Favorites"
                  : collection === "standalone"
                    ? "Standalone pages"
                    : "Your wiki"}
            </h1>
            <p>
              {trash
                ? "Select a deleted page to inspect its history or restore it."
                : collection === "favorites"
                  ? "Your starred pages in this namespace."
                  : collection === "standalone"
                    ? "Pages with neither a parent nor child pages."
                    : "Select a page or create your first one."}
            </p>
            {collection && (
              <div
                class="wiki-collection-results"
                aria-label={
                  collection === "favorites"
                    ? "Favorite pages"
                    : "Standalone pages"
                }
              >
                {list.items.map((p) => (
                  <button
                    onClick={() => navigate(`/wiki/${namespace}/page/${p.id}`)}
                  >
                    <strong>{p.name}</strong>
                  </button>
                ))}
                {!list.items.length && <p>No pages in this collection.</p>}
              </div>
            )}
          </div>
        )}
        {missing && !editing && (
          <ModalDialog
            title="Missing wiki page"
            onClose={() => setMissing(null)}
          >
            <div class="wiki-missing-content">
              <h2>{missing}</h2>
              <p>This page has not been created.</p>
              <button
                disabled={!canEdit}
                onClick={() => {
                  const next = crypto.randomUUID();
                  navigate(
                    `/wiki/${namespace}/new/${next}/${encodeURIComponent(missing)}`,
                  );
                }}
              >
                Create page
              </button>
              <button onClick={() => setMissing(null)}>Close</button>
            </div>
          </ModalDialog>
        )}
      </main>
    </section>
  );
}
