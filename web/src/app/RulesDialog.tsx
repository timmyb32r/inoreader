import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { ApiClient, RuleApplicationAccepted, RuleApplicationStatus, RuleApplicationStatusName, RuleDraft, RulePreview } from "../api/client";
import { AutofillResistantField, AutofillResistantSelect } from "../ui/fields";
import { Icon } from "../ui/Icon";
import { Dialog } from "./Dialog";

const terminal = new Set<RuleApplicationStatusName>(["completed", "cancelled", "failed"]);

export function RulesDialog({ client, workspaceId, subscriptionId, onClose }: { client: ApiClient; workspaceId: string; subscriptionId: string; onClose: () => void }) {
  const [rules, setRules] = useState<RuleDraft[]>([]);
  const [phrase, setPhrase] = useState("");
  const [field, setField] = useState<RuleDraft["field"]>("title_or_full_text");
  const [action, setAction] = useState<RuleDraft["action"]>("mark_read");
  const [preview, setPreview] = useState<RulePreview | null>(null);
  const [previewFingerprint, setPreviewFingerprint] = useState("");
  const [operation, setOperation] = useState<RuleApplicationAccepted | RuleApplicationStatus | null>(null);
  const [pending, setPending] = useState<"preview" | "save" | "apply" | "delete" | null>(null);
  const [error, setError] = useState("");
  const pollTimer = useRef<number>();
  const generation = useRef(0);
  const draft = useMemo<RuleDraft>(() => ({ subscriptionId, field, phrase, action, enabled: true }), [subscriptionId, field, phrase, action]);
  const fingerprint = JSON.stringify(draft);
  const previewCurrent = preview !== null && previewFingerprint === fingerprint;

  useEffect(() => {
    const current = ++generation.current; setRules([]); setPreview(null); setOperation(null); setError("");
    client.listRules(workspaceId).then(items => { if (current === generation.current) setRules(items); }).catch((failure: Error) => { if (current === generation.current) setError(failure.message); });
    return () => { generation.current++; if (pollTimer.current) window.clearTimeout(pollTimer.current); };
  }, [client, workspaceId]);

  const mutate = (work: () => void) => { work(); setError(""); };
  const refreshRules = (current: number) => client.listRules(workspaceId).then(items => { if (current === generation.current) setRules(items); });
  const poll = (operationId: string, current: number) => {
    if (current !== generation.current) return;
    client.ruleApplicationStatus(workspaceId, operationId).then(value => {
      if (current !== generation.current) return;
      setOperation(value);
      if (!terminal.has(value.status)) pollTimer.current = window.setTimeout(() => poll(operationId, current), 750);
    }).catch((failure: Error) => { if (current === generation.current) { setError(failure.message); setPending(null); } });
  };
  const runPreview = () => {
    if (pending || !phrase) return; const current = generation.current; setPending("preview"); setError("");
    client.previewRule(workspaceId, draft).then(value => { if (current === generation.current) { setPreview(value); setPreviewFingerprint(fingerprint); } }).catch((failure: Error) => { if (current === generation.current) setError(failure.message); }).finally(() => { if (current === generation.current) setPending(null); });
  };
  const save = (applyExisting: boolean) => {
    if (pending || !previewCurrent) return; const current = generation.current; setPending(applyExisting ? "apply" : "save"); setError(""); setOperation(null);
    client.saveRule(workspaceId, draft).then(async saved => {
      if (current !== generation.current) return;
      await refreshRules(current);
      if (!applyExisting || !saved.id) { setPhrase(""); setPreview(null); return; }
      const accepted = await client.applyRule(workspaceId, saved.id);
      if (current !== generation.current) return;
      setOperation(accepted); setPending(null); poll(accepted.operationId, current);
    }).catch((failure: Error) => { if (current === generation.current) setError(failure.message); }).finally(() => { if (current === generation.current) setPending(null); });
  };
  const remove = (rule: RuleDraft) => {
    if (!rule.id || pending || !window.confirm(`Delete the rule “${rule.phrase}”? Completed actions will not be reverted.`)) return; const current = generation.current; setPending("delete"); setError("");
    client.deleteRule(workspaceId, rule.id).then(() => { if (current === generation.current) setRules(items => items.filter(item => item.id !== rule.id)); }).catch((failure: Error) => { if (current === generation.current) setError(failure.message); }).finally(() => { if (current === generation.current) setPending(null); });
  };

  return <Dialog title="Subscription rules" description="Preview literal matching and shared-article scope before saving. Bulk apply is bounded to the server snapshot and reports durable progress." onClose={onClose} width="760px">
    <div class="modal__body rules-layout" aria-busy={pending !== null}>
      <section><div class="preview-articles"><small>ACTIVE RULES</small>{rules.length ? rules.map((rule, index) => <div key={rule.id ?? `${rule.phrase}-${index}`}><strong>v{index + 1} · {rule.phrase}</strong><span>{rule.field} · {rule.action}</span>{rule.id && <button class="icon-button icon-button--small" disabled={pending !== null} aria-label={`Delete ${rule.phrase}`} onClick={() => remove(rule)}><Icon name="trash" size={14}/></button>}</div>) : <p class="field-help">No rules for this workspace.</p>}</div></section>
      <section class="rule-editor"><label>New literal phrase<AutofillResistantField value={phrase} onInput={event => mutate(() => setPhrase(event.currentTarget.value))}/></label><div class="builder-pair"><label>Match field<AutofillResistantSelect value={field} onChange={event => mutate(() => setField(event.currentTarget.value as RuleDraft["field"]))}><option value="title">Title</option><option value="full_text">Full text</option><option value="title_or_full_text">Title or full text</option></AutofillResistantSelect></label><label>Action<AutofillResistantSelect value={action} onChange={event => mutate(() => setAction(event.currentTarget.value as RuleDraft["action"]))}><option value="mark_read">Mark read</option><option value="move_to_trash">Move to trash</option></AutofillResistantSelect></label></div>
        <button type="button" class="secondary-button rule-preview-button" disabled={pending !== null || !phrase} onClick={runPreview}>{pending === "preview" ? <><span class="spinner"/>Previewing…</> : "Preview matches"}</button>
        <RulePreviewPanel preview={previewCurrent ? preview : null} stale={preview !== null && !previewCurrent}/>
      </section>
      <OperationPanel operation={operation}/>
      <span class="field-help field-help--error" role="alert">{error}</span>
    </div>
    <footer class="modal__actions"><button class="secondary-button" disabled={pending !== null || !previewCurrent} onClick={() => save(true)}>{pending === "apply" ? <><span class="spinner"/>Starting…</> : "Save & apply existing"}</button><span/><button class="secondary-button" onClick={onClose}>Close</button><button class="primary-button" disabled={pending !== null || !previewCurrent} onClick={() => save(false)}>{pending === "save" ? <><span class="spinner"/>Saving…</> : "Save for new articles"}</button></footer>
  </Dialog>;
}

function RulePreviewPanel({ preview, stale }: { preview: RulePreview | null; stale: boolean }) {
  return <div class="rule-preview-panel" aria-live="polite">{preview ? <><div class="rule-stats"><span><strong>{preview.matchedArticles}</strong>Matched</span><span><strong>{preview.sharedArticles}</strong>Shared</span><span><strong>{preview.totalSubscriptionArticles}</strong>Total</span></div><p>{preview.sharedArticles > 0 ? `${preview.sharedArticles} matched article${preview.sharedArticles === 1 ? " is" : "s are"} shared with linked subscriptions. The action applies to the common article in every linked feed view.` : "No matched article is shared with another subscription."}</p><details><summary>Sample article IDs ({preview.sampleArticleIds.length})</summary><ul>{preview.sampleArticleIds.map(id => <li key={id}><code>{id}</code></li>)}</ul></details></> : <p>{stale ? "The rule changed. Preview it again before saving." : "Preview shows matches in this subscription and their scope across linked feeds."}</p>}</div>;
}

function OperationPanel({ operation }: { operation: RuleApplicationAccepted | RuleApplicationStatus | null }) {
  const evaluated = operation && "evaluated" in operation ? operation.evaluated : 0;
  const cancelReason = operation && "cancelReason" in operation ? operation.cancelReason : undefined;
  return <section class="rule-operation" aria-live="polite">{operation ? <><div><strong>Bulk apply: {operation.status}</strong><span>{evaluated} articles evaluated</span></div>{operation.status === "running" || operation.status === "queued" ? <progress aria-label="Bulk apply progress">Working</progress> : <span class={`rule-operation__result rule-operation__result--${operation.status}`}>{operation.status === "completed" ? "All articles within the fixed boundary were evaluated." : cancelReason ?? `Operation ${operation.status}.`}</span>}</> : <><strong>Bulk apply</strong><span>Progress and cancellation reasons will appear here.</span></>}</section>;
}
