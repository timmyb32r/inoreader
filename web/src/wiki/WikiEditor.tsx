import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import { ApiError } from "../api/client";
import type {
  WikiDraft,
  WikiLimits,
  WikiPage,
  WikiWrite,
} from "../api/generated";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import {
  AutofillResistantField as Field,
  AutofillResistantTextarea as Textarea,
} from "../ui/fields";
import { WikiMarkdown } from "./WikiMarkdown";
const draftKey = (d: WikiDraft) =>
  JSON.stringify([
    d.id,
    d.revision ?? null,
    d.page ?? null,
    d.base_revision ?? null,
    d.name,
    d.markdown,
  ]);
export function WikiEditor({
  client,
  namespace,
  id,
  page,
  initialName,
  limits,
  onSaved,
  onCancel,
  onDirty,
}: {
  client: WikiClient;
  namespace: string;
  id: string;
  page: WikiPage | null;
  initialName: string;
  limits: WikiLimits;
  onSaved: (p: WikiPage) => void;
  onCancel: () => void;
  onDirty: (v: boolean) => void;
}) {
  const [draft, setDraft] = useState<WikiDraft>({
    revision: null,
    id,
    page: page?.id ?? null,
    base_revision: page?.revision ?? null,
    name: page?.name ?? initialName,
    markdown: page?.markdown ?? "",
  });
  const [base, setBase] = useState<WikiPage | null>(page);
  const [publishing, setPublishing] = useState(false);
  const [ready, setReady] = useState(false),
    [status, setStatus] = useState("Loading draft…"),
    [preview, setPreview] = useState(false),
    [conflict, setConflict] = useState<WikiPage | null>(null);
  const saved = useRef<string>(""),
    latest = useRef(draft),
    alive = useRef(true),
    savingDraft = useRef(false),
    draftRequest = useRef<Promise<WikiDraft> | null>(null),
    followupTimer = useRef<number>(),
    writing = useRef(false),
    retry = useRef<WikiWrite | null>(null);
  latest.current = draft;
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  useEffect(() => {
    alive.current = true;
    client
      .draft(namespace, id)
      .then(async ({ draft: stored }) => {
        if (!alive.current) return;
        const initial = stored ?? latest.current;
        if (
          initial.page &&
          initial.base_revision &&
          initial.base_revision !== page?.revision
        ) {
          const revision = await client.revision(
            namespace,
            initial.page,
            initial.base_revision,
          );
          if (!alive.current) return;
          setBase(revision.page);
        }
        saved.current = draftKey(initial);
        setDraft(initial);
        setReady(true);
        setStatus(stored ? "Private draft restored" : "");
      })
      .catch(report);
    return () => {
      alive.current = false;
      clearTimeout(followupTimer.current);
      onDirty(false);
    };
  }, []);
  const persist = async () => {
    if (!ready || savingDraft.current || writing.current) return;
    const snapshot = latest.current,
      serialized = draftKey(snapshot);
    if (serialized === saved.current) return;
    savingDraft.current = true;
    setStatus("Saving private draft…");
    try {
      draftRequest.current = client.saveDraft(namespace, snapshot);
      const stored = await draftRequest.current;
      if (alive.current) {
        saved.current = draftKey(stored);
        latest.current = { ...latest.current, revision: stored.revision };
        setDraft(latest.current);
        const dirty = draftKey(latest.current) !== saved.current;
        onDirty(dirty);
        setStatus(dirty ? "Unsaved changes" : "Private draft saved");
      }
    } catch (e) {
      if (alive.current) report(e);
    } finally {
      savingDraft.current = false;
      draftRequest.current = null;
      if (
        alive.current &&
        !writing.current &&
        JSON.stringify({ ...latest.current, revision: snapshot.revision }) !==
          serialized
      )
        followupTimer.current = window.setTimeout(
          () => void persist(),
          limits.draft_save_delay_ms,
        );
    }
  };
  useEffect(() => {
    if (!ready) return;
    const dirty = draftKey(draft) !== saved.current;
    onDirty(dirty);
    if (!dirty) return;
    setStatus("Unsaved changes");
    const timer = window.setTimeout(
      () => void persist(),
      limits.draft_save_delay_ms,
    );
    return () => clearTimeout(timer);
  }, [draft, ready]);
  useEffect(() => {
    const unload = (event: BeforeUnloadEvent) => {
      if (draftKey(latest.current) !== saved.current) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", unload);
    return () => window.removeEventListener("beforeunload", unload);
  }, []);
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  const save = async () => {
    if (writing.current) return;
    writing.current = true;
    setPublishing(true);
    onDirty(true);
    try {
      if (draftRequest.current) await draftRequest.current;
      const d = latest.current;
      if (
        !d.name ||
        /[\n\r\[\]\0]/.test(d.name) ||
        bytes(d.name) > limits.name_bytes
      )
        throw new Error(
          "Enter a valid name within the configured byte limit; brackets and line breaks are not allowed.",
        );
      if (
        bytes(d.markdown) > limits.markdown_bytes ||
        d.markdown.includes("\0")
      )
        throw new Error(
          "Markdown exceeds the configured byte limit or contains NUL.",
        );
      const candidate: WikiWrite = {
        operation: crypto.randomUUID(),
        page: id,
        expected_revision: d.base_revision ?? null,
        change: { action: "save", name: d.name, markdown: d.markdown },
      };
      const prior = retry.current;
      const command =
        prior &&
        JSON.stringify(prior.change) === JSON.stringify(candidate.change) &&
        prior.expected_revision === candidate.expected_revision
          ? prior
          : candidate;
      retry.current = command;
      setStatus("Publishing…");
      const published = await client.write(namespace, command);
      saved.current = draftKey(d);
      onDirty(false);
      retry.current = null;
      // Published text is durable even if deleting its private draft fails.
      try {
        await client.discard(namespace, id, d.revision ?? null);
      } catch {
        setStatus("Published; the private draft remains saved.");
      }
      if (alive.current) onSaved(published);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        try {
          const current = await client.page(namespace, id);
          setConflict(current);
          setStatus(
            "Conflict: compare versions below. Your draft is preserved.",
          );
        } catch {
          report(e);
        }
      } else report(e);
    } finally {
      writing.current = false;
      if (alive.current) {
        setPublishing(false);
        onDirty(draftKey(latest.current) !== saved.current);
      }
    }
  };
  return (
    <section class="wiki-editor" aria-label="Wiki editor">
      <div class="wiki-editor__toolbar">
        <AsyncButton
          onPress={save}
          onError={report}
          disabled={!ready || publishing}
        >
          Save
        </AsyncButton>
        <button disabled={publishing} onClick={onCancel}>
          Cancel
        </button>
        <button aria-pressed={preview} onClick={() => setPreview(!preview)}>
          {preview ? "Write" : "Preview"}
        </button>
      </div>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      <Field
        aria-label="Page name"
        value={draft.name}
        disabled={!ready || publishing}
        onInput={(e) => setDraft({ ...draft, name: e.currentTarget.value })}
      />
      <div class={`wiki-editor__panes${preview ? " is-preview" : ""}`}>
        <Textarea
          aria-label="Markdown"
          value={draft.markdown}
          disabled={!ready || publishing}
          onInput={(e) =>
            setDraft({ ...draft, markdown: e.currentTarget.value })
          }
        />
        <div class="wiki-editor__preview">
          <WikiMarkdown
            text={draft.markdown}
            onLink={() =>
              setStatus("Save the page before opening an internal link.")
            }
          />
        </div>
      </div>
      {conflict && (
        <section class="wiki-conflict" aria-label="Revision conflict">
          <h3>Compare before saving</h3>
          <div class="wiki-comparison">
            <div>
              <h4>Original revision: {base?.name}</h4>
              <pre>{base?.markdown ?? "New page"}</pre>
            </div>
            <div>
              <h4>Current published revision: {conflict.name}</h4>
              <pre>{conflict.markdown}</pre>
            </div>
            <div>
              <h4>Your draft</h4>
              <pre>{draft.markdown}</pre>
            </div>
          </div>
          <button
            onClick={() => {
              setDraft({
                ...draft,
                page: conflict.id,
                base_revision: conflict.revision,
              });
              setConflict(null);
              retry.current = null;
              setStatus(
                "Current revision selected as the base. Review your text, then Save.",
              );
            }}
          >
            Use current revision as base
          </button>
        </section>
      )}
    </section>
  );
}
