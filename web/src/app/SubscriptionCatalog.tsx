import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import {
  AutofillResistantField,
  AutofillResistantSelect,
  AutofillResistantTextarea,
} from "../ui/fields";
import { Icon } from "../ui/Icon";
import { ModalDialog } from "../ui/ModalDialog";
import type { Subscription } from "../api/viewModels";
import {
  compareSubscriptions,
  defaultWidths,
  formatKind,
  isProblem,
  labels,
  move,
  readLayout,
  readSort,
  toggle,
  writeLayout,
  writeSort,
  type CatalogView,
  type Column,
  type Layout,
  type SortDirection,
} from "./subscriptionCatalogModel";
import { SubscriptionIcon } from "./SubscriptionIcon";
export function SubscriptionCatalog({
  client,
  workspaceId,
  workspaceName,
  subscriptions,
  onBack,
  onRefresh,
  onPause,
  onOpenDetail,
  onChanged,
}: {
  client: ApiClient;
  workspaceId: string;
  workspaceName: string;
  subscriptions: Subscription[];
  onBack: () => void;
  onRefresh: (id: string) => Promise<void>;
  onPause: (id: string) => void;
  onOpenDetail?: (id: string) => void;
  onChanged?: (item: Subscription) => void;
}) {
  const key = `reader.subscription-columns.${workspaceId}`;
  const initial = readLayout(key);
  const sortKey = `reader.subscription-sort.${workspaceId}`;
  const initialSort = readSort(sortKey);
  const [view, setView] = useState<CatalogView>("current"),
    [query, setQuery] = useState(""),
    [status, setStatus] = useState("all"),
    [kind, setKind] = useState("all"),
    [health, setHealth] = useState("all"),
    [sort, setSort] = useState<Column>(initialSort.column),
    [sortDirection, setSortDirection] = useState<SortDirection>(
      initialSort.direction,
    ),
    [selected, setSelected] = useState<Set<string>>(() => new Set()),
    [pending, setPending] = useState(false),
    [results, setResults] = useState<Map<string, string>>(() => new Map()),
    [layout, setLayout] = useState<Layout>(initial),
    [draft, setDraft] = useState<Layout>(initial),
    [editing, setEditing] = useState(false),
    [pauseOpen, setPauseOpen] = useState(false),
    [pauseReason, setPauseReason] = useState(""),
    [pauseError, setPauseError] = useState("");
  const pendingRef = useRef(false);
  useEffect(() => {
    const next = readLayout(key),
      nextSort = readSort(sortKey);
    setLayout(next);
    setDraft(next);
    setSort(nextSort.column);
    setSortDirection(nextSort.direction);
    setSelected(new Set());
    setResults(new Map());
  }, [key, sortKey]);
  const rows = useMemo(
    () =>
      subscriptions
        .filter((item) =>
          view === "archived"
            ? item.status === "archived"
            : view === "attention"
              ? item.status !== "archived" && isProblem(item)
              : item.status !== "archived",
        )
        .filter((item) => status === "all" || item.status === status)
        .filter((item) => kind === "all" || item.sourceType === kind)
        .filter(
          (item) =>
            health === "all" || (health === "problem") === isProblem(item),
        )
        .filter((item) =>
          `${item.name} ${item.sourceUrl ?? ""} ${item.personalNote ?? ""}`
            .toLocaleLowerCase()
            .includes(query.toLocaleLowerCase()),
        )
        .sort((a, b) => compareSubscriptions(a, b, sort, sortDirection)),
    [subscriptions, view, status, kind, health, query, sort, sortDirection],
  );
  const changeSort = (column: Column) => {
    const direction: SortDirection =
      sort === column && sortDirection === "asc" ? "desc" : "asc";
    setSort(column);
    setSortDirection(direction);
    writeSort(sortKey, column, direction);
  };
  const runBulk = async (
    ids: string[],
    work: (id: string) => Promise<void>,
    success: string,
  ) => {
    if (pendingRef.current || !ids.length) return;
    pendingRef.current = true;
    setPending(true);
    setResults((old) => {
      const next = new Map(old);
      ids.forEach((id) => next.set(id, "Pending…"));
      return next;
    });
    const settled = await Promise.allSettled(ids.map(work));
    setResults((old) => {
      const next = new Map(old);
      settled.forEach((result, index) =>
        next.set(
          ids[index],
          result.status === "fulfilled"
            ? success
            : result.reason instanceof Error
              ? result.reason.message
              : "Failed",
        ),
      );
      return next;
    });
    window.setTimeout(() => {
      pendingRef.current = false;
      setPending(false);
    }, 250);
  };
  const refresh = () => runBulk([...selected], onRefresh, "Refresh queued");
  const applyColumns = () => {
    setLayout(draft);
    writeLayout(key, draft);
    setEditing(false);
  };
  return (
    <section class="subscriptions-page">
      <header class="entity-header">
        <div>
          <button class="text-button" onClick={onBack}>
            ← Back
          </button>
          <p class="eyebrow">{workspaceName}</p>
          <h1>Subscriptions</h1>
          <p>Browse, inspect and maintain every source in this workspace.</p>
        </div>
      </header>
      <nav class="entity-tabs" aria-label="Subscription views">
        {(["current", "attention", "archived"] as const).map((item) => (
          <button
            class={view === item ? "active" : ""}
            onClick={() => {
              setView(item);
              setSelected(new Set());
            }}
          >
            {item === "attention"
              ? "Needs attention"
              : item[0].toUpperCase() + item.slice(1)}
          </button>
        ))}
      </nav>
      <div class="catalog-toolbar">
        <AutofillResistantField
          aria-label="Search subscriptions"
          type="search"
          placeholder="Search name, URL or personal note"
          value={query}
          onInput={(e) => setQuery(e.currentTarget.value)}
        />
        <AutofillResistantSelect
          aria-label="Filter by status"
          value={status}
          onChange={(e) => setStatus(e.currentTarget.value)}
        >
          <option value="all">All statuses</option>
          <option value="active">Active</option>
          <option value="paused">Paused</option>
          <option value="archived">Archived</option>
        </AutofillResistantSelect>
        <AutofillResistantSelect
          aria-label="Filter by type"
          value={kind}
          onChange={(e) => setKind(e.currentTarget.value)}
        >
          <option value="all">All types</option>
          <option value="feed">Feed</option>
          <option value="web">Web feed</option>
          <option value="built_in">Built in</option>
        </AutofillResistantSelect>
        <AutofillResistantSelect
          aria-label="Filter by health"
          value={health}
          onChange={(e) => setHealth(e.currentTarget.value)}
        >
          <option value="all">All health</option>
          <option value="problem">Problem feeds</option>
          <option value="healthy">Healthy feeds</option>
        </AutofillResistantSelect>
        <AutofillResistantSelect
          aria-label="Sort subscriptions"
          value={sort}
          onChange={(e) => {
            const next = e.currentTarget.value as Column;
            setSort(next);
            writeSort(sortKey, next, sortDirection);
          }}
        >
          {(Object.keys(labels) as Column[]).map((column) => (
            <option value={column}>{labels[column]}</option>
          ))}
        </AutofillResistantSelect>
        <button
          class="secondary-button"
          onClick={() => {
            setDraft(layout);
            setEditing(true);
          }}
        >
          Columns
        </button>
      </div>
      <div class="bulk-bar" aria-live="polite">
        <span>
          {selected.size
            ? `${selected.size} selected`
            : `${rows.length} subscriptions`}
        </span>
        <button
          disabled={!selected.size || pending}
          aria-busy={pending}
          onClick={refresh}
        >
          {pending ? (
            <>
              <span class="spinner" />
              Refreshing…
            </>
          ) : (
            "Refresh"
          )}
        </button>
        <button
          disabled={!selected.size || pending}
          onClick={() => setPauseOpen(true)}
        >
          Pause
        </button>
      </div>
      <div class="subscription-table-wrap">
        <table class="subscription-table">
          <colgroup>
            <col style="width:42px" />
            {layout.columns.map((c) => (
              <col style={`width:${layout.widths[c] ?? defaultWidths[c]}px`} />
            ))}
            <col style="width:110px" />
          </colgroup>
          <thead>
            <tr>
              <th>
                <AutofillResistantField
                  type="checkbox"
                  aria-label="Select all visible subscriptions"
                  checked={
                    !!rows.length && rows.every((r) => selected.has(r.id))
                  }
                  onChange={(e) =>
                    setSelected(
                      e.currentTarget.checked
                        ? new Set(rows.map((r) => r.id))
                        : new Set(),
                    )
                  }
                />
              </th>
              {layout.columns.map((c) => (
                <th
                  aria-sort={
                    sort === c
                      ? sortDirection === "asc"
                        ? "ascending"
                        : "descending"
                      : undefined
                  }
                >
                  <button
                    class="table-sort"
                    data-direction={sort === c ? sortDirection : "none"}
                    onClick={() => changeSort(c)}
                  >
                    {labels[c]}
                  </button>
                </th>
              ))}
              <th aria-label="Actions" />
            </tr>
          </thead>
          <tbody>
            {rows.map((item) => (
              <tr>
                <td>
                  <AutofillResistantField
                    type="checkbox"
                    aria-label={`Select ${item.name}`}
                    checked={selected.has(item.id)}
                    onChange={() => setSelected((old) => toggle(old, item.id))}
                  />
                </td>
                {layout.columns.map((c) => (
                  <td data-label={labels[c]}>
                    {cell(item, c, onOpenDetail)}
                    {c === "name" && results.has(item.id) && (
                      <small
                        class={`bulk-result ${results.get(item.id) === "Pending…" ? "pending" : results.get(item.id)?.includes("queued") || results.get(item.id) === "Paused" ? "success" : "error"}`}
                      >
                        {results.get(item.id)}
                      </small>
                    )}
                  </td>
                ))}
                <td class="row-link">
                  <a
                    href={`/subscriptions/${encodeURIComponent(item.id)}`}
                    onClick={(e) => {
                      if (onOpenDetail) {
                        e.preventDefault();
                        onOpenDetail(item.id);
                      }
                    }}
                  >
                    View details
                  </a>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {!rows.length && (
          <div class="catalog-empty">
            <Icon name="feed" />
            <strong>No subscriptions in this view</strong>
            <span>Try another filter or search term.</span>
          </div>
        )}
      </div>
      {editing && (
        <div
          class="column-drawer"
          role="dialog"
          aria-label="Choose table columns"
        >
          <h2>Table columns</h2>
          <p>Choose, order and size fields. Changes apply when you confirm.</p>
          {(Object.keys(labels) as Column[]).map((c) => {
            const shown = draft.columns.includes(c),
              index = draft.columns.indexOf(c);
            return (
              <div class="column-option">
                <label>
                  <AutofillResistantField
                    type="checkbox"
                    disabled={c === "name" || c === "attention"}
                    checked={shown}
                    onChange={() =>
                      setDraft((old) => ({
                        ...old,
                        columns: shown
                          ? old.columns.filter((x) => x !== c)
                          : [...old.columns, c],
                      }))
                    }
                  />
                  {labels[c]}
                </label>
                {shown && (
                  <>
                    <div class="column-order">
                      <button
                        aria-label={`Move ${labels[c]} up`}
                        disabled={index < 1}
                        onClick={() =>
                          setDraft((old) => ({
                            ...old,
                            columns: move(old.columns, index, index - 1),
                          }))
                        }
                      >
                        ↑
                      </button>
                      <button
                        aria-label={`Move ${labels[c]} down`}
                        disabled={index === draft.columns.length - 1}
                        onClick={() =>
                          setDraft((old) => ({
                            ...old,
                            columns: move(old.columns, index, index + 1),
                          }))
                        }
                      >
                        ↓
                      </button>
                    </div>
                    <AutofillResistantSelect
                      aria-label={`${labels[c]} width`}
                      value={String(draft.widths[c] ?? defaultWidths[c])}
                      onChange={(e) =>
                        setDraft((old) => ({
                          ...old,
                          widths: {
                            ...old.widths,
                            [c]: Number(e.currentTarget.value),
                          },
                        }))
                      }
                    >
                      <option value={Math.max(80, defaultWidths[c] - 60)}>
                        Narrow
                      </option>
                      <option value={defaultWidths[c]}>Default</option>
                      <option value={defaultWidths[c] + 100}>Wide</option>
                    </AutofillResistantSelect>
                  </>
                )}
              </div>
            );
          })}
          <footer>
            <button class="secondary-button" onClick={() => setEditing(false)}>
              Cancel
            </button>
            <button class="primary-button" onClick={applyColumns}>
              Apply columns
            </button>
          </footer>
        </div>
      )}
      {pauseOpen && (
        <ModalDialog
          title="Pause selected subscriptions"
          description="One required reason will be recorded for every selected subscription."
          onClose={() => {
            if (!pending) setPauseOpen(false);
          }}
        >
          <div class="modal__body">
            <label>
              Reason <span class="required">Required</span>
              <AutofillResistantTextarea
                rows={4}
                value={pauseReason}
                onInput={(e) => setPauseReason(e.currentTarget.value)}
                autoFocus
              />
            </label>
            <span class="field-help field-help--error" role="alert">
              {pauseError}
            </span>
          </div>
          <footer class="modal__actions">
            <span />
            <span />
            <button
              class="secondary-button"
              disabled={pending}
              onClick={() => setPauseOpen(false)}
            >
              Cancel
            </button>
            <button
              class="danger-button"
              disabled={pending || !pauseReason.trim()}
              aria-busy={pending}
              onClick={() => {
                if (pending || !pauseReason.trim()) return;
                setPauseError("");
                runBulk(
                  [...selected],
                  async (id) => {
                    await client.pauseSubscription(id, pauseReason);
                    const current = subscriptions.find(
                      (item) => item.id === id,
                    );
                    if (current)
                      onChanged?.({
                        ...current,
                        status: "paused",
                        reason: pauseReason,
                        reasonAt: new Date().toISOString(),
                      });
                  },
                  "Paused",
                )
                  .then(() => {
                    setPauseOpen(false);
                    setSelected(new Set());
                    setPauseReason("");
                  })
                  .catch((e: Error) => setPauseError(e.message));
              }}
            >
              {pending ? (
                <>
                  <span class="spinner" />
                  Pausing…
                </>
              ) : (
                "Pause subscriptions"
              )}
            </button>
          </footer>
        </ModalDialog>
      )}
    </section>
  );
}
function cell(s: Subscription, c: Column, onOpenDetail?: (id: string) => void) {
  switch (c) {
    case "attention":
      return isProblem(s) ? (
        <span
          class="subscription-attention"
          title={s.attentionReason || "Needs attention"}
        >
          <span aria-hidden="true">!</span>Needs attention
        </span>
      ) : (
        <span class="subscription-healthy">—</span>
      );
    case "name":
      return (
        <a
          class="entity-name entity-name--with-icon"
          href={`/subscriptions/${encodeURIComponent(s.id)}`}
          onClick={(e) => {
            if (onOpenDetail) {
              e.preventDefault();
              onOpenDetail(s.id);
            }
          }}
        >
          <SubscriptionIcon
            name={s.name}
            iconDataUrl={s.iconDataUrl}
            size={30}
          />
          <span>
            {s.name}
            {s.attentionReason && <small>{s.attentionReason}</small>}
          </span>
        </a>
      );
    case "kind":
      return formatKind(s.sourceType);
    case "status":
      return <span class={`status-pill status-${s.status}`}>{s.status}</span>;
    case "updated":
      return s.lastUpdate || "Never";
    case "unread":
      return s.unreadCount ?? 0;
    case "url":
      return s.sourceUrl || "—";
    case "note":
      return s.personalNote || "—";
    case "interval":
      return s.pollingInterval || "—";
    case "error":
      return s.error || "—";
    case "added":
      return s.createdAt || "—";
  }
}
