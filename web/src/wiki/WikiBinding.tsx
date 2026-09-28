import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type {
  WikiBinding as Binding,
  WikiNamespaces,
  WikiPages,
} from "../api/generated";
import { AsyncButton } from "../ui/AsyncButton";
import { ModalDialog } from "../ui/ModalDialog";
import { StatusRegion } from "../ui/StatusRegion";
import {
  AutofillResistantSelect as Select,
  AutofillResistantField as Field,
} from "../ui/fields";
import "./wiki.css";
export function WikiBinding({
  client,
  subscription,
  onOpen,
}: {
  client: WikiClient;
  subscription: string;
  onOpen?: (path: string) => boolean;
}) {
  const [binding, setBinding] = useState<Binding | null>(null),
    [open, setOpen] = useState(false),
    [status, setStatus] = useState("");
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  useEffect(() => {
    let active = true;
    client
      .binding(subscription)
      .then((b) => {
        if (active) setBinding(b);
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, [subscription]);
  return (
    <div class="wiki-binding">
      <div class="wiki-binding__row">
        <span title={binding?.page?.name}>
          {binding?.page?.name ??
            (binding?.linked ? "Page unavailable" : "Not linked")}
        </span>
        {binding?.page && binding.namespace ? (
          <a
            onClick={(event) => {
              if (
                onOpen &&
                !event.metaKey &&
                !event.ctrlKey &&
                !event.shiftKey &&
                !event.altKey &&
                event.button === 0
              ) {
                event.preventDefault();
                onOpen(`/wiki/${binding.namespace}/page/${binding.page!.id}`);
              }
            }}
            href={`/wiki/${binding.namespace}/page/${binding.page.id}?return=${encodeURIComponent(location.pathname + location.search)}`}
          >
            Open
          </a>
        ) : (
          <span />
        )}
        <button disabled={!binding} onClick={() => setOpen(true)}>
          {binding?.linked ? "Change" : "Link page"}
        </button>
        <AsyncButton
          disabled={!binding?.linked}
          onError={report}
          onPress={async () => {
            await client.bind(subscription, { target: null });
            setBinding({ linked: false, page: null, namespace: null });
            setStatus("Page unlinked");
          }}
        >
          Unlink
        </AsyncButton>
      </div>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      {open && (
        <ModalDialog
          title="Link wiki page"
          description="Choose an existing page. No subscription content is shared."
          onClose={() => setOpen(false)}
          width="620px"
        >
          <BindingPicker
            client={client}
            subscription={subscription}
            onSaved={(b) => {
              setBinding(b);
              setOpen(false);
              setStatus("Page linked");
            }}
          />
        </ModalDialog>
      )}
    </div>
  );
}
function BindingPicker({
  client,
  subscription,
  onSaved,
}: {
  client: WikiClient;
  subscription: string;
  onSaved: (b: Binding) => void;
}) {
  const epoch = useRef(0);
  useEffect(
    () => () => {
      epoch.current++;
    },
    [],
  );
  const [namespaces, setNamespaces] = useState<WikiNamespaces>({
      items: [],
      has_more: false,
    }),
    [pages, setPages] = useState<WikiPages>({ items: [], has_more: false }),
    [namespace, setNamespace] = useState(""),
    [page, setPage] = useState(""),
    [search, setSearch] = useState(""),
    [status, setStatus] = useState("Loading namespaces…"),
    [size, setSize] = useState(0),
    [nsOffset, setNsOffset] = useState(0),
    [offset, setOffset] = useState(0);
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  useEffect(() => {
    let active = true;
    Promise.all([client.namespaces(), client.limits()])
      .then(([n, l]) => {
        if (active) {
          setNamespaces(n);
          setSize(l.page_size);
          setStatus("");
        }
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, []);
  useEffect(() => {
    setPages({ items: [], has_more: false });
    setPage("");
    setOffset(0);
    if (!namespace) return;
    let active = true;
    setStatus("Loading pages…");
    client
      .pages(namespace)
      .then((p) => {
        if (active) {
          setPages(p);
          setStatus("");
        }
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, [namespace]);
  const loadPages = async (nextOffset: number) => {
    const token = ++epoch.current;
    const result = await client.pages(namespace, search, false, nextOffset);
    if (token !== epoch.current) return;
    setPages(result);
    setOffset(nextOffset);
    setPage("");
  };
  return (
    <div class="wiki-shell wiki-picker">
      <Select
        aria-label="Wiki namespace"
        value={namespace}
        onChange={(e) => {
          epoch.current++;
          setNamespace(e.currentTarget.value);
        }}
      >
        <option value="">Choose namespace</option>
        {namespaces.items.map((n) => (
          <option value={n.id}>{n.name}</option>
        ))}
      </Select>
      <div class="wiki-pagination">
        <AsyncButton
          disabled={!nsOffset}
          onError={report}
          onPress={async () => {
            setNamespaces(await client.namespaces(nsOffset - size));
            setNsOffset(nsOffset - size);
          }}
        >
          Previous namespaces
        </AsyncButton>
        <AsyncButton
          disabled={!namespaces.has_more}
          onError={report}
          onPress={async () => {
            setNamespaces(await client.namespaces(nsOffset + size));
            setNsOffset(nsOffset + size);
          }}
        >
          Next namespaces
        </AsyncButton>
      </div>
      <div class="wiki-search">
        <Field
          aria-label="Find wiki page"
          value={search}
          onInput={(e) => setSearch(e.currentTarget.value)}
        />
        <AsyncButton
          disabled={!namespace}
          onError={report}
          onPress={() => loadPages(0)}
        >
          Find page
        </AsyncButton>
      </div>
      <Select
        aria-label="Wiki page"
        value={page}
        onChange={(e) => setPage(e.currentTarget.value)}
      >
        <option value="">Choose page</option>
        {pages.items.map((p) => (
          <option value={p.id}>{p.name}</option>
        ))}
      </Select>
      <div class="wiki-pagination">
        <AsyncButton
          disabled={!offset}
          onError={report}
          onPress={() => loadPages(offset - size)}
        >
          Previous pages
        </AsyncButton>
        <AsyncButton
          disabled={!pages.has_more}
          onError={report}
          onPress={() => loadPages(offset + size)}
        >
          Next pages
        </AsyncButton>
      </div>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      <AsyncButton
        disabled={!namespace || !page}
        onError={report}
        onPress={async () => {
          await client.bind(subscription, { target: { namespace, page } });
          onSaved(await client.binding(subscription));
        }}
      >
        Link page
      </AsyncButton>
    </div>
  );
}
