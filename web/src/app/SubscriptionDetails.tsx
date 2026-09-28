import { useSubscriptionLifecycle } from "./useSubscriptionLifecycle";
import { useEffect, useRef, useState } from "preact/hooks";
import type {
  ApiClient,
  RuleDraft,
  SourceUrlPreview,
  SubscriptionActivity,
  SubscriptionDetail,
  SubscriptionExtraction,
  WebFeedRecipeView,
} from "../api/client";
import {
  AutofillResistantField,
  AutofillResistantTextarea,
} from "../ui/fields";
import { ModalDialog } from "../ui/ModalDialog";
import type { Subscription } from "../api/viewModels";
import { LatestArticles } from "./LatestArticles";
import { PublicationHistory } from "./PublicationHistory";
import { formatKind } from "./subscriptionCatalogModel";
export type DetailTab = "overview" | "extraction" | "rules" | "activity";
export function SubscriptionDetails({
  client,
  workspaceId,
  workspaceName,
  subscriptions,
  subscriptionId,
  initialTab,
  onBack,
  onOpenArticles,
  onRefresh,
  onPause,
  onChanged,
  onRemoved,
  onEditRecipe,
  onDirtyNoteChange,
}: {
  client: ApiClient;
  workspaceId: string;
  workspaceName: string;
  subscriptions: Subscription[];
  subscriptionId: string;
  initialTab?: DetailTab;
  onBack: () => void;
  onOpenArticles: (id: string, articleId?: string) => void;
  onRefresh: (id: string) => Promise<void>;
  onPause: (id: string) => void;
  onChanged?: (item: Subscription) => void;
  onRemoved?: (id: string) => void;
  onEditRecipe?: (recipe: WebFeedRecipeView) => void;
  onDirtyNoteChange?: (dirty: boolean) => void;
}) {
  const summary = subscriptions.find((x) => x.id === subscriptionId);
  const [detail, setDetail] = useState<SubscriptionDetail | null>(
      summary ?? null,
    ),
    [tab, setTab] = useState(initialTab ?? "overview"),
    [note, setNote] = useState(summary?.personalNote ?? ""),
    [savedNote, setSavedNote] = useState(summary?.personalNote ?? ""),
    [saving, setSaving] = useState(false),
    [refreshing, setRefreshing] = useState(false),
    [error, setError] = useState(""),
    [activity, setActivity] = useState<SubscriptionActivity[] | null>(null),
    [activityLoading, setActivityLoading] = useState(false),
    [extraction, setExtraction] = useState<SubscriptionExtraction | null>(null),
    [extractionLoading, setExtractionLoading] = useState(false),
    [extractionAttempted, setExtractionAttempted] = useState(false),
    [rules, setRules] = useState<RuleDraft[] | null>(null),
    [rulesLoading, setRulesLoading] = useState(false),
    [nameEditing, setNameEditing] = useState(false),
    [customName, setCustomName] = useState(summary?.customName ?? ""),
    [urlOpen, setUrlOpen] = useState(false),
    [newUrl, setNewUrl] = useState(summary?.sourceUrl ?? ""),
    [urlPreview, setUrlPreview] = useState<SourceUrlPreview | null>(null),
    [urlPending, setUrlPending] = useState(false);
  const lifecycle = useSubscriptionLifecycle(
    client,
    (item) => {
      setDetail((old) => (old ? { ...old, ...item } : item));
      onChanged?.(item);
    },
    onRemoved,
    onBack,
  );
  const noteEdited = useRef(false);
  const dirty = note !== savedNote;
  useEffect(() => {
    let active = true;
    client
      .getSubscription(subscriptionId)
      .then((v) => {
        if (active && v) {
          setDetail(v);
          if (!noteEdited.current) setNote(v.personalNote ?? "");
          setSavedNote(v.personalNote ?? "");
        }
      })
      .catch((e) => {
        if (active) setError(e.message);
      });
    return () => {
      active = false;
    };
  }, [client, subscriptionId]);
  useEffect(() => {
    const before = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", before);
    return () => window.removeEventListener("beforeunload", before);
  }, [dirty]);
  useEffect(() => {
    onDirtyNoteChange?.(dirty);
    return () => onDirtyNoteChange?.(false);
  }, [dirty, onDirtyNoteChange]);
  useEffect(() => {
    if (tab !== "activity" || activity !== null || activityLoading) return;
    setActivityLoading(true);
    client
      .subscriptionActivity(subscriptionId)
      .then(setActivity)
      .catch((e) => {
        setError(e.message);
        setActivity([]);
      })
      .finally(() => setActivityLoading(false));
  }, [tab, activity, activityLoading, client, subscriptionId]);
  useEffect(() => {
    if (tab !== "extraction" || extractionAttempted || extractionLoading)
      return;
    setExtractionAttempted(true);
    setExtractionLoading(true);
    client
      .subscriptionExtraction(subscriptionId)
      .then(setExtraction)
      .catch((e) => setError(e.message))
      .finally(() => setExtractionLoading(false));
  }, [tab, extractionAttempted, extractionLoading, client, subscriptionId]);
  useEffect(() => {
    if (tab !== "rules" || rules !== null || rulesLoading) return;
    setRulesLoading(true);
    client
      .listRules(workspaceId)
      .then((values) =>
        setRules(
          values.filter((rule) => rule.subscriptionId === subscriptionId),
        ),
      )
      .catch((e) => {
        setError(e.message);
        setRules([]);
      })
      .finally(() => setRulesLoading(false));
  }, [tab, rules, rulesLoading, client, workspaceId, subscriptionId]);
  const leave = (action: () => void) => {
    if (
      !dirty ||
      window.confirm("Discard unsaved changes to your personal note?")
    )
      action();
  };
  if (!detail)
    return (
      <section class="subscriptions-page">
        <button class="text-button" onClick={onBack}>
          ← Back
        </button>
        <div class="catalog-empty">
          <strong>Subscription not found</strong>
        </div>
      </section>
    );
  const save = () => {
    if (saving) return;
    setSaving(true);
    setError("");
    client
      .saveSubscriptionNote(subscriptionId, note)
      .then((v) => {
        setDetail(v);
        noteEdited.current = false;
        setSavedNote(note);
      })
      .catch((e) => setError(e.message))
      .finally(() => setSaving(false));
  };
  return (
    <section class="subscriptions-page detail-page">
      {lifecycle.dialog}
      <header class="entity-header">
        <div>
          <button class="text-button" onClick={onBack}>
            ← Back
          </button>
          <p class="eyebrow">{workspaceName} · Subscription</p>
          <h1>{detail.name}</h1>
          <p>
            {detail.sourceTitle && detail.sourceTitle !== detail.name
              ? `Source title: ${detail.sourceTitle}`
              : "Source details and collection health"}
          </p>
        </div>
        <div class="entity-actions">
          {detail.status === "archived" ? (
            <button
              class="secondary-button"
              onClick={() => lifecycle.open("restore", [detail])}
            >
              Restore subscription
            </button>
          ) : (
            <>
              <button
                class="secondary-button"
                disabled={refreshing}
                onClick={() => {
                  setRefreshing(true);
                  onRefresh(detail.id).finally(() => setRefreshing(false));
                }}
              >
                {refreshing ? "Queuing…" : "Refresh"}
              </button>
              <button
                class="secondary-button"
                onClick={() => onPause(detail.id)}
              >
                Pause
              </button>
            </>
          )}
          {detail.status !== "archived" && (
            <button
              class="secondary-button"
              onClick={() => lifecycle.open("archive", [detail])}
            >
              Archive
            </button>
          )}
          <button
            class="secondary-button"
            onClick={() => lifecycle.open("delete", [detail])}
          >
            Delete
          </button>
        </div>
      </header>
      <nav class="entity-tabs" aria-label="Subscription details">
        {(
          [
            ["overview", "Overview"],
            ["extraction", "Extraction"],
            ["rules", "Rules"],
            ["activity", "Update log"],
          ] satisfies [DetailTab, string][]
        ).map(([id, label]) => (
          <button
            class={tab === id ? "active" : ""}
            onClick={() => leave(() => setTab(id))}
          >
            {label}
          </button>
        ))}
      </nav>
      {tab === "overview" && (
        <div class="detail-grid">
          <section class="detail-card">
            <div class="card-heading">
              <h2>Source</h2>
              <button
                class="text-button"
                onClick={() => {
                  setCustomName(detail.customName ?? "");
                  setNameEditing(true);
                }}
              >
                Edit name
              </button>
            </div>
            <dl>
              <dt>Custom name</dt>
              <dd>{detail.customName || "Not set"}</dd>
              <dt>Source title</dt>
              <dd>{detail.sourceTitle || detail.name}</dd>
              <dt>URL</dt>
              <dd class="source-url">
                <span>{detail.sourceUrl || "Not reported"}</span>
                {detail.sourceUrl && (
                  <>
                    <a href={detail.sourceUrl} target="_blank" rel="noreferrer">
                      Open
                    </a>
                    <button
                      onClick={() =>
                        navigator.clipboard.writeText(detail.sourceUrl!)
                      }
                    >
                      Copy
                    </button>
                  </>
                )}
                {detail.sourceType !== "web" && (
                  <button
                    onClick={() => {
                      setNewUrl(detail.sourceUrl ?? "");
                      setUrlPreview(null);
                      setUrlOpen(true);
                    }}
                  >
                    Change source URL
                  </button>
                )}
              </dd>
              <dt>Status</dt>
              <dd>
                {detail.status}
                {detail.attentionReason && (
                  <small class="attention-reason">
                    {detail.attentionReason}
                  </small>
                )}
              </dd>
              <dt>Last updated</dt>
              <dd>
                {detail.lastUpdate
                  ? new Date(detail.lastUpdate).toLocaleString()
                  : "Not updated yet"}
              </dd>
              <dt>Articles</dt>
              <dd>{detail.count}</dd>
              <dt>Unread</dt>
              <dd>{detail.unreadCount ?? 0}</dd>
            </dl>
          </section>
          <section class="detail-card note-card">
            <h2>Personal note</h2>
            <AutofillResistantTextarea
              aria-label="Personal note"
              rows={7}
              value={note}
              onInput={(e) => {
                noteEdited.current = true;
                setNote(e.currentTarget.value);
              }}
            />
            <div class="note-status" aria-live="polite">
              <span>{dirty ? "Unsaved changes" : "All changes saved"}</span>
              <button
                class="primary-button"
                disabled={!dirty || saving}
                aria-busy={saving}
                onClick={save}
              >
                {saving ? (
                  <>
                    <span class="spinner" />
                    Saving…
                  </>
                ) : (
                  "Save note"
                )}
              </button>
            </div>
            <span class="field-help field-help--error">{error}</span>
          </section>
          <PublicationHistory client={client} subscriptionId={subscriptionId} />
          <LatestArticles
            client={client}
            workspaceId={workspaceId}
            subscriptionId={detail.id}
            onOpenArticles={onOpenArticles}
          />
        </div>
      )}
      {tab === "extraction" && (
        <section
          class="detail-card detail-single"
          aria-busy={extractionLoading}
        >
          <div class="card-heading">
            <div>
              <h2>Extraction</h2>
              {extraction?.recipeSummary && <p>{extraction.recipeSummary}</p>}
            </div>
            {detail.editableWebFeed && (
              <button
                class="primary-button"
                onClick={() =>
                  client
                    .getWebFeedRecipe(detail.id)
                    .then((recipe) => onEditRecipe?.(recipe))
                    .catch((e) => setError(e.message))
                }
              >
                Edit extraction recipe
              </button>
            )}
          </div>
          {extractionLoading ? (
            <p>Loading extraction details…</p>
          ) : (
            extraction && (
              <dl>
                <dt>Source type</dt>
                <dd>
                  {formatKind(
                    extraction.sourceType as Subscription["sourceType"],
                  )}
                </dd>
                <dt>Discovered feed URLs</dt>
                <dd>
                  {extraction.feedUrls.length
                    ? extraction.feedUrls.join(", ")
                    : "No feed URLs discovered"}
                </dd>
                {extraction.recipeVersion !== undefined && (
                  <>
                    <dt>Recipe version</dt>
                    <dd>{extraction.recipeVersion}</dd>
                  </>
                )}
                <dt>Last preview</dt>
                <dd>
                  {extraction.lastPreview
                    ? new Date(extraction.lastPreview).toLocaleString()
                    : "No preview recorded"}
                </dd>
              </dl>
            )
          )}
        </section>
      )}
      {tab === "rules" && (
        <section class="detail-card detail-single" aria-busy={rulesLoading}>
          <h2>Rules</h2>
          {rulesLoading ? (
            <p>Loading rules…</p>
          ) : (
            <RuleGroup
              title="Subscription rules"
              rules={(rules ?? []).map((rule) => ({
                id: rule.id ?? `${rule.field}:${rule.phrase}`,
                summary: `${rule.field.replaceAll("_", " ")} contains “${rule.phrase}” → ${rule.action.replaceAll("_", " ")}`,
                enabled: rule.enabled,
              }))}
            />
          )}
          <RuleGroup
            title="Workspace-level rules"
            rules={[]}
            empty="Workspace-level rules are not supported yet."
          />
        </section>
      )}
      {tab === "activity" && (
        <section class="detail-card detail-single">
          <div class="card-heading">
            <div>
              <h2>Update log · last 30 days</h2>
              <p>
                Every recorded fetch attempt for this subscription, including
                the full failure reason.
              </p>
            </div>
          </div>
          <div class="activity-list" aria-busy={activityLoading}>
            {activityLoading ? (
              <p>Loading update log…</p>
            ) : activity?.length ? (
              activity.map((a) => (
                <article>
                  <span
                    class={`status-pill status-${a.successful ? "success" : "error"}`}
                  >
                    {a.successful ? "Success" : "Failed"}
                  </span>
                  <strong>{new Date(a.occurredAt).toLocaleString()}</strong>
                  <span>
                    {a.durationMs === undefined
                      ? "Duration unavailable"
                      : `${a.durationMs} ms`}{" "}
                    ·{" "}
                    {a.discoveredItems === undefined
                      ? "Items unavailable"
                      : `${a.discoveredItems} items`}
                  </span>
                  {a.diagnostic && (
                    <p class="activity-diagnostic">{a.diagnostic}</p>
                  )}
                </article>
              ))
            ) : (
              <p>No update attempts recorded in the last 30 days.</p>
            )}
          </div>
        </section>
      )}
      {nameEditing && (
        <ModalDialog
          title="Edit custom name"
          description="The source title remains available and continues updating from the feed."
          onClose={() => !saving && setNameEditing(false)}
        >
          <div class="modal__body">
            <label>
              Custom name
              <AutofillResistantField
                aria-label="Custom name"
                value={customName}
                onInput={(e) => setCustomName(e.currentTarget.value)}
              />
            </label>
          </div>
          <footer class="modal__actions">
            <span />
            <span />
            <button
              class="secondary-button"
              disabled={saving}
              onClick={() => setNameEditing(false)}
            >
              Cancel
            </button>
            <button
              class="primary-button"
              disabled={saving || customName.trim() !== customName}
              onClick={() => {
                setSaving(true);
                client
                  .renameSubscription(detail.id, customName)
                  .then((item) => {
                    setDetail((old) => (old ? { ...old, ...item } : old));
                    onChanged?.(item);
                    setNameEditing(false);
                  })
                  .catch((e) => setError(e.message))
                  .finally(() => setSaving(false));
              }}
            >
              Save name
            </button>
          </footer>
        </ModalDialog>
      )}
      {urlOpen && (
        <ModalDialog
          title="Change source URL"
          description="The current source stays active until you preview and confirm the replacement."
          onClose={() => !urlPending && setUrlOpen(false)}
        >
          <div class="modal__body source-url-flow">
            <label>
              New source URL
              <AutofillResistantField
                aria-label="New source URL"
                type="url"
                value={newUrl}
                onInput={(e) => {
                  setNewUrl(e.currentTarget.value);
                  setUrlPreview(null);
                }}
              />
            </label>
            {urlPreview && (
              <div class="source-preview" aria-live="polite">
                <span>Preview ready</span>
                <strong>{urlPreview.title}</strong>
                <small>{urlPreview.url}</small>
              </div>
            )}
            <span class="field-help field-help--error" role="alert">
              {error}
            </span>
          </div>
          <footer class="modal__actions">
            <button
              class="secondary-button"
              disabled={urlPending}
              onClick={() => setUrlOpen(false)}
            >
              Cancel
            </button>
            <span />
            <button
              class="secondary-button"
              disabled={urlPending || !newUrl || !!urlPreview}
              aria-busy={urlPending && !urlPreview}
              onClick={() => {
                setUrlPending(true);
                setError("");
                client
                  .previewSubscriptionSourceUrl(detail.id, newUrl)
                  .then(setUrlPreview)
                  .catch((e) => setError(e.message))
                  .finally(() => setUrlPending(false));
              }}
            >
              {urlPending && !urlPreview ? "Previewing…" : "Preview source"}
            </button>
            <button
              class="primary-button"
              disabled={urlPending || !urlPreview}
              aria-busy={urlPending && !!urlPreview}
              onClick={() => {
                if (!urlPreview) return;
                setUrlPending(true);
                client
                  .commitSubscriptionSourceUrl(detail.id, urlPreview.token)
                  .then((item) => {
                    setDetail((old) => (old ? { ...old, ...item } : old));
                    onChanged?.(item);
                    setUrlOpen(false);
                  })
                  .catch((e) => setError(e.message))
                  .finally(() => setUrlPending(false));
              }}
            >
              {urlPending && urlPreview ? "Changing…" : "Confirm change"}
            </button>
          </footer>
        </ModalDialog>
      )}
    </section>
  );
}
function RuleGroup({
  title,
  rules,
  empty = "No rules in this group.",
}: {
  title: string;
  rules: {
    id: string;
    summary: string;
    enabled: boolean;
  }[];
  empty?: string;
}) {
  return (
    <div class="rule-group">
      <h3>{title}</h3>
      {rules.length ? (
        rules.map((r) => (
          <div>
            <span>{r.summary}</span>
            <small>{r.enabled ? "Enabled" : "Disabled"}</small>
          </div>
        ))
      ) : (
        <p>{empty}</p>
      )}
    </div>
  );
}
