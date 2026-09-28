import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type { WikiLimits, WikiNamespaces } from "../api/generated";
import { AutofillResistantField as Field } from "../ui/fields";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import { NamespaceWorkspace } from "./NamespaceWorkspace";
import "./wiki.css";
export function WikiApplication({
  client,
  path,
  navigate,
  onDirty,
  onBack,
}: {
  client: WikiClient;
  path: string;
  navigate: (path: string) => boolean;
  onDirty: (dirty: boolean) => void;
  onBack: () => void;
}) {
  const [limits, setLimits] = useState<WikiLimits | null>(null),
    [namespaces, setNamespaces] = useState<WikiNamespaces>({
      items: [],
      has_more: false,
    }),
    [offset, setOffset] = useState(0),
    [name, setName] = useState(""),
    [status, setStatus] = useState("Loading wiki…");
  const createId = useRef(crypto.randomUUID());
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Wiki request failed");
  useEffect(() => {
    let active = true;
    Promise.all([client.limits(), client.namespaces()])
      .then(([l, n]) => {
        if (active) {
          setLimits(l);
          setNamespaces(n);
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
  const parts = path.split("/").filter(Boolean),
    ns = parts[1];
  if (ns && limits)
    return (
      <NamespaceWorkspace
        key={ns}
        {...{ client, limits, navigate, onDirty, onBack }}
        namespace={ns}
        parts={parts}
      />
    );
  return (
    <section class="wiki-shell wiki-landing">
      <header>
        <button onClick={onBack}>← Back to Reader</button>
        <h1>Wiki</h1>
        <p>Private pages, shared only with people you choose.</p>
      </header>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      <div class="wiki-create">
        <Field
          aria-label="Namespace name"
          placeholder="Namespace name"
          value={name}
          onInput={(e) => setName(e.currentTarget.value)}
        />
        <AsyncButton
          disabled={!limits || !name}
          onError={report}
          onPress={async () => {
            if (
              limits &&
              (new TextEncoder().encode(name).length > limits.name_bytes ||
                /[\n\r\[\]\0]/.test(name))
            )
              throw new Error("Invalid namespace name");
            const n = await client.createNamespace(createId.current, name);
            navigate(`/wiki/${n.id}`);
          }}
        >
          New namespace
        </AsyncButton>
      </div>
      <div class="wiki-namespaces">
        {namespaces.items.map((n) => (
          <button onClick={() => navigate(`/wiki/${n.id}`)}>
            <strong>{n.name}</strong>
            <span>{n.role}</span>
          </button>
        ))}
      </div>
      <div class="wiki-pagination">
        <AsyncButton
          disabled={!limits || !offset}
          onError={report}
          onPress={async () => {
            if (!limits) return;
            const o = offset - limits.page_size;
            setNamespaces(await client.namespaces(o));
            setOffset(o);
          }}
        >
          Previous
        </AsyncButton>
        <AsyncButton
          disabled={!limits || !namespaces.has_more}
          onError={report}
          onPress={async () => {
            if (!limits) return;
            const o = offset + limits.page_size;
            setNamespaces(await client.namespaces(o));
            setOffset(o);
          }}
        >
          Next
        </AsyncButton>
      </div>
    </section>
  );
}
