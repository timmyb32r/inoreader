import { useEffect, useRef, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type { Namespace, WikiWrite } from "../api/generated";
import { ModalDialog } from "../ui/ModalDialog";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import {
  AutofillResistantField as Field,
  AutofillResistantSelect as Select,
} from "../ui/fields";
import "./wiki.css";

/** Retries retain the write operation and page identity after uncertain responses.
 * A failed binding leaves the created page intact and retries only the binding. */
export function CreateSubscriptionPage({
  client,
  subscription,
  name: initialName,
  onClose,
  onCreated,
}: {
  client: WikiClient;
  subscription: string;
  name: string;
  onClose: () => void;
  onCreated: (path: string) => void;
}) {
  const [namespaces, setNamespaces] = useState<Namespace[]>([]);
  const [namespace, setNamespace] = useState("");
  const [name, setName] = useState(initialName);
  const [status, setStatus] = useState("Loading namespaces…");
  const [busy, setBusy] = useState(false);
  const [started, setStarted] = useState(false);
  const attempt = useRef<{
    namespace: string;
    command: WikiWrite;
    created: boolean;
    parentCommand?: WikiWrite;
    parented?: boolean;
  } | null>(null);
  useEffect(() => {
    let active = true;
    (async () => {
      const limits = await client.limits();
      const all: Namespace[] = [];
      let offset = 0;
      for (;;) {
        const batch = await client.namespaces(offset);
        all.push(...batch.items.filter((n) => n.role !== "reader"));
        if (!batch.has_more) break;
        offset += limits.page_size;
      }
      if (active) {
        setNamespaces(all);
        if (all.length === 1) setNamespace(all[0].id);
        setStatus(
          all.length
            ? ""
            : "Create a wiki namespace first, or ask for editing access.",
        );
      }
    })().catch((e) => {
      if (active)
        setStatus(e instanceof Error ? e.message : "Could not load namespaces");
    });
    return () => {
      active = false;
    };
  }, [client]);
  return (
    <ModalDialog
      title="Create subscription wiki page"
      description="Choose where to create the page. It will be linked to this subscription."
      onClose={() => {
        if (!busy) onClose();
      }}
      width="620px"
    >
      <div class="wiki-shell wiki-picker">
        <Select
          aria-label="Wiki namespace"
          value={namespace}
          disabled={started}
          onChange={(e) => setNamespace(e.currentTarget.value)}
        >
          <option value="">Choose namespace</option>
          {namespaces.map((n) => (
            <option key={n.id} value={n.id}>
              {n.name}
            </option>
          ))}
        </Select>
        <Field
          aria-label="Wiki page name"
          value={name}
          disabled={started}
          onInput={(e) => setName(e.currentTarget.value)}
        />
        <StatusRegion class="wiki-status">{status}</StatusRegion>
        <AsyncButton
          disabled={!namespace || !name}
          onError={(e) =>
            setStatus(
              e instanceof Error ? e.message : "Could not create or link page",
            )
          }
          onPress={async () => {
            setBusy(true);
            setStarted(true);
            try {
              const current: NonNullable<typeof attempt.current> =
                attempt.current ?? {
                  namespace,
                  command: {
                    operation: crypto.randomUUID(),
                    page: crypto.randomUUID(),
                    expected_revision: null,
                    change: { action: "save" as const, name, markdown: "" },
                  },
                  created: false,
                };
              attempt.current = current;
              if (!current.created) {
                const root = await client.subscriptionRoot(current.namespace);
                const page = await client.write(
                  current.namespace,
                  current.command,
                );
                current.parentCommand = {
                  operation: crypto.randomUUID(),
                  page: page.id,
                  expected_revision: page.revision,
                  change: { action: "set_parent", parent: root.id },
                };
                current.created = true;
              }
              if (!current.parented && current.parentCommand) {
                await client.write(current.namespace, current.parentCommand);
                current.parented = true;
              }
              setStatus("Page created; linking…");
              await client.bind(subscription, {
                target: {
                  namespace: current.namespace,
                  page: current.command.page,
                },
              });
              onCreated(
                `/wiki/${current.namespace}/page/${current.command.page}`,
              );
            } finally {
              setBusy(false);
            }
          }}
        >
          Create and link
        </AsyncButton>
      </div>
    </ModalDialog>
  );
}
