import { useEffect, useRef, useState } from "preact/hooks";
import type { AiClient, AiProfile } from "../api/ai";

/** Profile mutations may finish after the profile dialog closes. Accept their
 * result only for the original signed-in account, and fence older initial reads. */
export function useAiProfile(client: AiClient, accountId: string) {
  const [state, setState] = useState<{
    owner: string;
    profile: AiProfile;
  } | null>(null);
  const owner = useRef(accountId),
    revision = useRef(0);
  owner.current = accountId;
  useEffect(() => {
    const token = ++revision.current;
    setState(null);
    if (accountId)
      void client
        .profile()
        .then((value) => {
          if (owner.current === accountId && revision.current === token)
            setState({ owner: accountId, profile: value });
        })
        .catch(() => {
          /* Profile renders errors explicitly; reading stays available. */
        });
    return () => {
      revision.current += 1;
    };
  }, [client, accountId]);
  const update = (requestOwner: string, value: AiProfile) => {
    if (requestOwner !== owner.current) return;
    revision.current += 1;
    setState({ owner: requestOwner, profile: value });
  };
  return { profile: state?.owner === accountId ? state.profile : null, update };
}
