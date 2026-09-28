import { useEffect, useState } from "preact/hooks";
import type { WikiClient } from "../api/wiki";
import type { WikiMembers } from "../api/generated";
import { AsyncButton } from "../ui/AsyncButton";
import { StatusRegion } from "../ui/StatusRegion";
import {
  AutofillResistantField as Field,
  AutofillResistantSelect as Select,
} from "../ui/fields";
export function WikiAccess({
  client,
  namespace,
  pageSize,
}: {
  client: WikiClient;
  namespace: string;
  pageSize: number;
}) {
  const [data, setData] = useState<WikiMembers>({ items: [], has_more: false }),
    [offset, setOffset] = useState(0),
    [name, setName] = useState(""),
    [role, setRole] = useState<"reader" | "editor">("reader"),
    [status, setStatus] = useState("");
  const report = (e: unknown) =>
    setStatus(e instanceof Error ? e.message : "Request failed");
  const reload = async (o = offset) => {
    const d = await client.members(namespace, o);
    setData(d);
    setOffset(o);
  };
  useEffect(() => {
    let active = true;
    client
      .members(namespace)
      .then((d) => {
        if (active) setData(d);
      })
      .catch((e) => {
        if (active) report(e);
      });
    return () => {
      active = false;
    };
  }, []);
  return (
    <section class="wiki-access">
      <h2>Namespace access</h2>
      <p>
        Only the owner manages access. Reader subscriptions are never shared.
      </p>
      <div class="wiki-access__form">
        <Field
          aria-label="Existing username"
          placeholder="Existing username"
          value={name}
          onInput={(e) => setName(e.currentTarget.value)}
        />
        <Select
          aria-label="Wiki role"
          value={role}
          onChange={(e) => setRole(e.currentTarget.value as typeof role)}
        >
          <option value="reader">Reader</option>
          <option value="editor">Editor</option>
        </Select>
        <AsyncButton
          disabled={!name}
          onError={report}
          onPress={async () => {
            await client.setMember(namespace, { username: name, role });
            await reload();
            setStatus("Access saved");
          }}
        >
          Grant access
        </AsyncButton>
      </div>
      <StatusRegion class="wiki-status">{status}</StatusRegion>
      <ul>
        {data.items.map((m) => (
          <li key={m.account}>
            <span>
              {m.username} · {m.role}
            </span>
            <AsyncButton
              onError={report}
              onPress={async () => {
                if (!confirm(`Revoke access for ${m.username}?`)) return;
                await client.setMember(namespace, {
                  username: m.username,
                  role: null,
                });
                await reload();
                setStatus("Access revoked");
              }}
            >
              Revoke
            </AsyncButton>
          </li>
        ))}
      </ul>
      <div class="wiki-pagination">
        <AsyncButton
          disabled={!offset}
          onError={report}
          onPress={() => reload(offset - pageSize)}
        >
          Previous
        </AsyncButton>
        <AsyncButton
          disabled={!data.has_more}
          onError={report}
          onPress={() => reload(offset + pageSize)}
        >
          Next
        </AsyncButton>
      </div>
    </section>
  );
}
