import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient, ArticlePage } from "../api/client";
import type { Subscription } from "../api/viewModels";

/** Load both halves before publishing a workspace. Unmount invalidates the entire request. */
export function useWorkspaceSwitch(
  client: ApiClient,
  currentId: string,
  apply: (id: string, page: ArticlePage, subscriptions: Subscription[]) => void,
  announce: (message: string) => void,
  waitForWrites: () => Promise<void>,
) {
  const generation = useRef(0),
    locked = useRef(false);
  const [switchingWorkspace, setSwitchingWorkspace] = useState(false);
  useEffect(
    () => () => {
      generation.current++;
    },
    [],
  );
  const switchWorkspace = async (id: string) => {
    if (id === currentId || locked.current) return;
    locked.current = true;
    setSwitchingWorkspace(true);
    const request = ++generation.current;
    try {
      await waitForWrites();
      if (request !== generation.current) return;
      const [page, subscriptions] = await Promise.all([
        client.listArticles(id, "feed"),
        client.listSubscriptions(id),
      ]);
      if (request === generation.current) apply(id, page, subscriptions);
    } catch (error) {
      if (request === generation.current) announce((error as Error).message);
    } finally {
      if (request === generation.current) {
        locked.current = false;
        setSwitchingWorkspace(false);
      }
    }
  };
  return { switchingWorkspace, switchWorkspace };
}
