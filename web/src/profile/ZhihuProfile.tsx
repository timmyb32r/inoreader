import { useEffect, useRef, useState } from "preact/hooks";
import type { ZhihuClient } from "../api/zhihu";
import type { ZhihuProfileView } from "../api/generated";
import { AutofillResistantTextarea } from "../ui/fields";
import "./zhihu.css";

export function ZhihuProfile({ client }: { client: ZhihuClient }) {
  const [profile, setProfile] = useState<ZhihuProfileView | null>(null);
  const [cookies, setCookies] = useState("");
  const [pending, setPending] = useState<string | null>("Loading");
  const [message, setMessage] = useState("");
  const [confirm, setConfirm] = useState(false);
  const locked = useRef(false),
    epoch = useRef(0);
  useEffect(() => {
    const current = ++epoch.current;
    client
      .status()
      .then((p) => {
        if (current === epoch.current) setProfile(p);
      })
      .catch((e) => {
        if (current === epoch.current) setMessage(e.message);
      })
      .finally(() => {
        if (current === epoch.current) setPending(null);
      });
    return () => {
      epoch.current++;
    };
  }, [client]);
  async function run(action: "Save" | "Check" | "Remove") {
    if (locked.current || pending) return;
    locked.current = true;
    setPending(action);
    setMessage("");
    const current = epoch.current;
    try {
      const result = await (action === "Save"
        ? client.save(cookies)
        : action === "Check"
          ? client.check()
          : client.remove());
      if (current !== epoch.current) return;
      setProfile(result);
      if (action !== "Check") setCookies("");
      setConfirm(false);
      setMessage(
        action === "Save"
          ? "Session verified and saved. Existing Zhihu subscriptions queued."
          : action === "Check"
            ? "Zhihu session is valid."
            : "Session removed. Collected articles kept.",
      );
    } catch (e) {
      if (current === epoch.current) setMessage((e as Error).message);
    } finally {
      if (current === epoch.current) {
        locked.current = false;
        setPending(null);
      }
    }
  }
  return (
    <section class="zhihu-profile" aria-labelledby="zhihu-title">
      <h3 id="zhihu-title">Zhihu</h3>
      <p class="zhihu-profile__state">
        {profile?.configured ? "Session saved" : "No session saved"}
      </p>
      <label for="zhihu-cookies">Zhihu cookies</label>
      <AutofillResistantTextarea
        id="zhihu-cookies"
        class="zhihu-profile__secret"
        value={cookies}
        disabled={!!pending || !profile?.available}
        onInput={(e) => setCookies(e.currentTarget.value)}
        aria-describedby="zhihu-help"
        spellcheck={false}
      />
      <p id="zhihu-help">
        Paste the Cookie header or the full browser cookie table, including
        z_c0. Encrypted in storage. Sent directly to Zhihu only.
      </p>
      <div class="zhihu-profile__actions">
        {(["Save", "Check", "Remove"] as const).map((action) => (
          <button
            class="secondary-button"
            disabled={
              !!pending ||
              !profile?.available ||
              (action === "Save" ? !cookies : !profile?.configured)
            }
            aria-busy={pending === action}
            onClick={() =>
              action === "Remove" ? setConfirm(true) : void run(action)
            }
          >
            <span style={{ opacity: pending === action ? 0 : 1 }}>
              {action}
            </span>
            {pending === action && (
              <span class="async-button__pending">
                <span class="spinner" />
              </span>
            )}
          </button>
        ))}
      </div>
      <div class="zhihu-profile__confirmation">
        {confirm && (
          <>
            <span>Remove session? Articles will be kept.</span>
            <button
              class="secondary-button"
              disabled={!!pending}
              onClick={() => void run("Remove")}
            >
              Confirm removal
            </button>
            <button
              class="secondary-button"
              disabled={!!pending}
              onClick={() => setConfirm(false)}
            >
              Cancel
            </button>
          </>
        )}
      </div>
      <div class="zhihu-profile__feedback" role="status" aria-live="polite">
        {pending === "Loading"
          ? "Loading…"
          : profile && !profile.available
            ? "Session encryption is not configured on this server."
            : message}
      </div>
    </section>
  );
}
