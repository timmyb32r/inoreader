import { useEffect, useRef, useState } from "preact/hooks";
import type { AiClient, AiProfile } from "../api/ai";
import { AutofillResistantField } from "../ui/fields";
import { formatArticleDate } from "../ui/formatArticleDate";
import "./deepseek.css";

export function DeepSeekProfile({ client, onProfile, completedGeneration }: {
  client: AiClient;
  onProfile: (profile: AiProfile) => void;
  completedGeneration: number;
}) {
  const [profile, setProfile] = useState<AiProfile | null>(null);
  const [key, setKey] = useState("");
  const [pending, setPending] = useState("Loading DeepSeek profile");
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const locked = useRef(false);
  const alive = useRef(true);
  const onProfileRef = useRef(onProfile);
  onProfileRef.current = onProfile;

  const apply = (next: AiProfile) => {
    setProfile(next);
    onProfileRef.current(next);
  };
  const run = async (label: string, request: () => Promise<AiProfile>, success: string) => {
    if (locked.current) return;
    locked.current = true;
    setPending(label);
    setError("");
    setStatus("");
    const publish = onProfileRef.current;
    try {
      const result = await request();
      publish(result);
      if (!alive.current) return;
      setProfile(result);
      setStatus(success);
    } catch (cause) {
      if (alive.current) setError(cause instanceof Error ? cause.message : "DeepSeek is unavailable. Your saved key is unchanged.");
    } finally {
      locked.current = false;
      if (alive.current) setPending("");
    }
  };
  useEffect(() => {
    alive.current = true;
    void run("Loading DeepSeek profile", async () => {
      const current = await client.profile();
      if (alive.current) apply(current);
      return alive.current && current.configured ? client.balance() : current;
    }, "");
    return () => { alive.current = false; };
  }, [client]);
  const previousGeneration = useRef(completedGeneration);
  useEffect(() => {
    if (previousGeneration.current === completedGeneration) return;
    previousGeneration.current = completedGeneration;
    if (profile?.configured && !document.hidden) void run("Refreshing balance", () => client.balance(), "Balance updated");
  }, [completedGeneration]);

  const save = () => {
    if (!key || locked.current) return;
    const submitted = key;
    void run("Checking and saving key", async () => {
      const next = await client.saveKey(submitted);
      if (alive.current) setKey("");
      return next;
    }, "API key checked and saved");
  };

  return <section class="ai-profile" aria-labelledby="deepseek-profile-title" aria-busy={!!pending}>
    <header><h3 id="deepseek-profile-title">DeepSeek</h3><span class="ai-profile__key-state">{profile?.configured ? "Key saved · ••••••••" : "No saved key"}</span></header>
    <p>Your key pays for your own summaries and chats. It is encrypted on the server and is never sent back to the browser.</p>
    <label>{profile?.configured ? "Replace DeepSeek API key" : "DeepSeek API key"}
      <AutofillResistantField type="password" value={key} spellcheck={false} disabled={!!pending} placeholder="Enter API key" onInput={event => setKey(event.currentTarget.value)} />
    </label>
    <div class="ai-profile__actions">
      <button class="primary-button" disabled={!!pending || !key} aria-busy={pending === "Checking and saving key"} onClick={save}>{pending === "Checking and saving key" ? <><span class="spinner"/> Saving…</> : "Save API key"}</button>
      <button class="secondary-button" disabled={!!pending || !profile?.configured} onClick={() => {
        if (locked.current || !window.confirm("Remove your DeepSeek key? Existing chats will be kept. New generations will be disabled.")) return;
        void run("Removing key", () => client.removeKey(), "API key removed; your chats are preserved");
      }}>{pending === "Removing key" ? <><span class="spinner"/> Removing…</> : "Remove key"}</button>
    </div>
    <div class="ai-profile__feedback" role="status" aria-live="polite">
      {pending ? <><span class="spinner"/> {pending}…</> : <span class={error || profile?.error ? "ai-error" : ""}>{error || profile?.error || status || profile?.availabilityReason || (profile?.enabled ? "Ready to summarize articles" : "Add a key to use DeepSeek")}</span>}
    </div>
    <div class="ai-balance">
      <header><h4>Balance</h4><button class="text-button" disabled={!!pending || !profile?.configured} onClick={() => void run("Refreshing balance", () => client.balance(), "Balance updated")}>Refresh balance</button></header>
      <div class="ai-balance__amounts">{profile?.balance ? <>
        {profile.balance.balances.map(item => <div key={item.currency}><strong>{item.total} {item.currency}</strong><small>Granted: {item.granted} · Topped up: {item.toppedUp}</small></div>)}
        <span class={profile.balance.available ? "" : "ai-error"}>{profile.balance.available ? "Funds available" : "Funds unavailable"}</span>
        <small>Last updated: {formatArticleDate(profile.balance.updatedAt)}</small>
      </> : <span>Balance will appear after your key is checked.</span>}</div>
      <p>Provider balance may include activity outside Reader.</p>
    </div>
  </section>;
}
