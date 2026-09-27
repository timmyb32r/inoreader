import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon";
import type { Workspace } from "./data";
export function WorkspacePicker({
  value,
  workspaces,
  archived,
  pending,
  onChange,
  onArchive,
}: {
  value: string;
  workspaces: Workspace[];
  archived: boolean;
  pending: boolean;
  onChange: (v: string) => void;
  onArchive: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div class="workspace-picker">
      <button
        class="workspace-button"
        disabled={pending}
        aria-busy={pending}
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        {pending ? (
          <span class="spinner" />
        ) : (
          <span class="workspace-glyph">{value.slice(0, 2).toUpperCase()}</span>
        )}
        <span>
          <small>Workspace</small>
          <strong>{pending ? "Switching…" : value}</strong>
        </span>
        <span class="chevron">⌄</span>
      </button>
      {open && (
        <div class="workspace-menu">
          {workspaces.map((item) => (
            <button
              key={item.id}
              disabled={pending}
              onClick={() => {
                onChange(item.id);
                setOpen(false);
              }}
            >
              <span class="workspace-glyph">{item.name.slice(0, 2)}</span>
              {item.name}
              {item.name === value && <Icon name="check" size={15} />}
            </button>
          ))}
          <hr />
          <button disabled={pending} onClick={onArchive}>
            <Icon name={archived ? "refresh" : "archive"} size={16} />
            {archived ? "Restore workspace" : "Archive workspace"}
          </button>
        </div>
      )}
    </div>
  );
}
