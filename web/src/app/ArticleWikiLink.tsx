import { useEffect, useState } from "preact/hooks";
import { CreateSubscriptionPage } from "../wiki/CreateSubscriptionPage";
import type { WikiClient } from "../api/wiki";

/** Reserve the action's footprint before the permission-scoped lookup finishes. */
export function ArticleWikiLink({
  client,
  subscription,
  name,
  onNavigate,
}: {
  client: WikiClient;
  subscription: string;
  name: string;
  onNavigate: (path: string) => void;
}) {
  const [unlinked, setUnlinked] = useState(false);
  const [creating, setCreating] = useState(false);
  const [target, setTarget] = useState<{ path: string; name: string } | null>(
    null,
  );
  useEffect(() => {
    let active = true;
    setTarget(null);
    setUnlinked(false);
    setCreating(false);
    client
      .binding(subscription)
      .then((binding) => {
        if (active) setUnlinked(!binding.linked);
        if (active && binding.namespace && binding.page)
          setTarget({
            path: `/wiki/${binding.namespace}/page/${binding.page.id}`,
            name: binding.page.name,
          });
      })
      .catch(() => {
        /* The optional shortcut never exposes inaccessible pages. */
      });
    return () => {
      active = false;
    };
  }, [client, subscription]);
  return (
    <span class="reader-wiki-slot">
      {unlinked && (
        <button
          class="reader-wiki-link"
          title="Create and link wiki page"
          aria-label="Create and link wiki page"
          onClick={() => setCreating(true)}
        >
          <span aria-hidden="true">W+</span>
        </button>
      )}
      {creating && (
        <CreateSubscriptionPage
          client={client}
          subscription={subscription}
          name={name}
          onClose={() => setCreating(false)}
          onCreated={(path) => {
            setCreating(false);
            setUnlinked(false);
            setTarget({ path, name });
            onNavigate(path);
          }}
        />
      )}
      {target && (
        <a
          class="reader-wiki-link"
          href={`${target.path}?return=${encodeURIComponent(location.pathname + location.search)}`}
          title={`Wiki: ${target.name}`}
          aria-label={`Open wiki page ${target.name}`}
          onClick={(event) => {
            if (
              event.button !== 0 ||
              event.metaKey ||
              event.ctrlKey ||
              event.shiftKey ||
              event.altKey
            )
              return;
            event.preventDefault();
            onNavigate(
              `${target.path}?return=${encodeURIComponent(location.pathname + location.search)}`,
            );
          }}
        >
          <span aria-hidden="true">W</span>
        </a>
      )}
    </span>
  );
}
