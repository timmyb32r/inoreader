import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type { Subscription } from "../api/viewModels";

/** Commands outlive dialogs. Results may update only the account/workspace that
 * accepted them; duplicate activations share a single in-flight operation.
 * Errors are returned to the initiating control's reserved status region. */
export function useSubscriptionCommands(
  client: ApiClient,
  accountId: string,
  workspaceId: string,
  changed: (subscription: Subscription) => void,
) {
  const scope = `${accountId}/${workspaceId}`;
  const active = useRef(scope);
  active.current = scope;
  const publish = useRef(changed);
  publish.current = changed;
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );
  const requests = useRef(new Map<string, Promise<boolean>>());
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  const run = (id: string, action: string, work: () => Promise<unknown>) => {
    const key = `${scope}/${id}/${action}`;
    const prior = requests.current.get(key);
    if (prior) return prior;
    setPending(new Set([...requests.current.keys(), key]));
    const request = Promise.resolve()
      .then(work)
      .then(async () => {
        const value = await client.getSubscription(id);
        if (!alive.current || active.current !== scope) return false;
        publish.current(value);
        return true;
      })
      .finally(() => {
        requests.current.delete(key);
        if (alive.current) setPending(new Set(requests.current.keys()));
      });
    requests.current.set(key, request);
    return request;
  };
  return {
    pending: pending.size > 0,
    pause: (id: string, reason: string) =>
      run(id, "pause", () => client.pauseSubscription(id, reason)),
    resume: (id: string) =>
      run(id, "resume", () => client.resumeSubscription(id)),
    refresh: (id: string) =>
      run(id, "refresh", () => client.refreshSubscription(id)),
    waitForWrites: async () => {
      while (requests.current.size)
        await Promise.allSettled(requests.current.values());
    },
  };
}
