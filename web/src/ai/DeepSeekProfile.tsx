import { useEffect, useRef, useState } from "preact/hooks";
import type { AiClient, AiProfile } from "../api/ai";
import { AutofillResistantField, AutofillResistantSelect } from "../ui/fields";
import { formatArticleDate } from "../ui/formatArticleDate";
import "./deepseek.css";
import { SpendingChart } from "./SpendingChart";

export function DeepSeekProfile({
  client,
  onProfile,
  completedGeneration,
}: {
  client: AiClient;
  onProfile: (profile: AiProfile) => void;
  completedGeneration: number;
}) {
  const [profile, setProfile] = useState<AiProfile | null>(null);
  const [models, setModels] = useState<AiProfile["models"]>({
    summary: "deepseek-flash",
    verification: "deepseek-flash",
  });
  useEffect(() => {
    if (profile) setModels(profile.models);
  }, [profile?.models.summary, profile?.models.verification]);
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
  const run = async (
    label: string,
    request: () => Promise<AiProfile>,
    success: string,
  ) => {
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
      if (alive.current)
        setError(
          cause instanceof Error
            ? cause.message
            : "DeepSeek is unavailable. Your saved key is unchanged.",
        );
    } finally {
      locked.current = false;
      if (alive.current) setPending("");
    }
  };
  useEffect(() => {
    alive.current = true;
    void run(
      "Loading DeepSeek profile",
      async () => {
        const current = await client.profile();
        if (alive.current) apply(current);
        return alive.current && current.configured ? client.balance() : current;
      },
      "",
    );
    return () => {
      alive.current = false;
    };
  }, [client]);
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (!document.hidden && !locked.current)
        void run("Refreshing spending", () => client.profile(), "");
    }, 30000);
    return () => window.clearInterval(timer);
  }, [client]);
  const previousGeneration = useRef(completedGeneration);
  useEffect(() => {
    if (previousGeneration.current === completedGeneration) return;
    previousGeneration.current = completedGeneration;
    if (profile?.configured && !document.hidden)
      void run("Refreshing balance", () => client.balance(), "Balance updated");
  }, [completedGeneration]);

  const save = () => {
    if (!key || locked.current) return;
    const submitted = key;
    void run(
      "Checking and saving key",
      async () => {
        const next = await client.saveKey(submitted);
        if (alive.current) setKey("");
        return next;
      },
      "API key checked and saved",
    );
  };

  return (
    <section
      class="ai-profile"
      aria-labelledby="deepseek-profile-title"
      aria-busy={!!pending}
    >
      <header>
        <h3 id="deepseek-profile-title">DeepSeek</h3>
        <span class="ai-profile__key-state">
          {profile?.configured ? "Key saved · ••••••••" : "No saved key"}
        </span>
      </header>
      <p>
        Your key pays for your own summaries and chats. It is encrypted on the
        server and is never sent back to the browser.
      </p>
      <label>
        {profile?.configured ? "Replace DeepSeek API key" : "DeepSeek API key"}
        <AutofillResistantField
          type="password"
          value={key}
          spellcheck={false}
          disabled={!!pending}
          placeholder="Enter API key"
          onInput={(event) => setKey(event.currentTarget.value)}
        />
      </label>
      <div class="ai-profile__actions">
        <button
          class="primary-button"
          disabled={!!pending || !key}
          aria-busy={pending === "Checking and saving key"}
          onClick={save}
        >
          {pending === "Checking and saving key" ? (
            <>
              <span class="spinner" /> Saving…
            </>
          ) : (
            "Save API key"
          )}
        </button>
        <button
          class="secondary-button"
          disabled={!!pending || !profile?.configured}
          onClick={() => {
            if (
              locked.current ||
              !window.confirm(
                "Remove your DeepSeek key? Existing chats will be kept. New generations will be disabled.",
              )
            )
              return;
            void run(
              "Removing key",
              () => client.removeKey(),
              "API key removed; your chats are preserved",
            );
          }}
        >
          {pending === "Removing key" ? (
            <>
              <span class="spinner" /> Removing…
            </>
          ) : (
            "Remove key"
          )}
        </button>
      </div>
      <div class="ai-profile__feedback" role="status" aria-live="polite">
        {pending ? (
          <>
            <span class="spinner" /> {pending}…
          </>
        ) : (
          <span class={error || profile?.error ? "ai-error" : ""}>
            {error ||
              profile?.error ||
              status ||
              profile?.availabilityReason ||
              (profile?.enabled
                ? "Ready to summarize articles"
                : "Add a key to use DeepSeek")}
          </span>
        )}
      </div>
      <div class="ai-model-settings">
        <h4>Models</h4>
        <div class="ai-model-settings__fields">
          {(["summary", "verification"] as const).map((stage) => (
            <label key={stage}>
              {stage === "summary" ? "Summary model" : "Fact-check model"}
              <AutofillResistantSelect
                value={models[stage]}
                disabled={!!pending || !profile}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  if (value === "deepseek-flash" || value === "deepseek-v4-pro")
                    setModels({ ...models, [stage]: value });
                }}
              >
                <option value="deepseek-flash">DeepSeek Flash · cheaper</option>
                <option value="deepseek-v4-pro">DeepSeek Pro</option>
              </AutofillResistantSelect>
            </label>
          ))}
        </div>
        <div class="ai-profile__actions">
          <button
            class="secondary-button"
            disabled={
              !!pending ||
              !profile ||
              (models.summary === profile.models.summary &&
                models.verification === profile.models.verification)
            }
            aria-busy={pending === "Saving models"}
            onClick={() =>
              void run(
                "Saving models",
                () => client.saveModels(models),
                "Models saved",
              )
            }
          >
            {pending === "Saving models" ? (
              <>
                <span class="spinner" /> Saving…
              </>
            ) : (
              "Save models"
            )}
          </button>
        </div>
        <p>
          Applies to the next request, including queued summaries. Running
          requests and saved summaries stay unchanged. Chat replies use the
          summary model.
        </p>
      </div>
      <div class="ai-balance">
        <header>
          <h4>Balance</h4>
          <button
            class="text-button"
            disabled={!!pending || !profile?.configured}
            onClick={() =>
              void run(
                "Refreshing balance",
                () => client.balance(),
                "Balance updated",
              )
            }
          >
            Refresh balance
          </button>
        </header>
        <div class="ai-balance__amounts">
          {profile?.balance ? (
            <>
              {profile.balance.balances.map((item) => (
                <div key={item.currency}>
                  <strong>
                    {item.total} {item.currency}
                  </strong>
                  <small>
                    Granted: {item.granted} · Topped up: {item.toppedUp}
                  </small>
                </div>
              ))}
              <span class={profile.balance.available ? "" : "ai-error"}>
                {profile.balance.available
                  ? "Funds available"
                  : "Funds unavailable"}
              </span>
              <small>
                Last updated: {formatArticleDate(profile.balance.updatedAt)}
              </small>
            </>
          ) : (
            <span>Balance will appear after your key is checked.</span>
          )}
        </div>
        <p>Provider balance may include activity outside Reader.</p>
      </div>
      <SpendingChart spending={profile?.spending} />
    </section>
  );
}
