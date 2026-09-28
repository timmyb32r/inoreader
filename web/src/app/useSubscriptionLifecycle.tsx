import { useRef, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type { Subscription } from "../api/viewModels";
import { ModalDialog } from "../ui/ModalDialog";
import { StatusRegion } from "../ui/StatusRegion";
import "./subscription-lifecycle.css";

type Action = "archive" | "restore" | "delete";
const labels = { archive: "Archive", restore: "Restore", delete: "Delete" };
/** The dialog stays mounted after rows disappear. Completion never exposes a new
 * row action underneath the initiating pointer; the user explicitly closes it. */
export function useSubscriptionLifecycle(
  client: ApiClient,
  onChanged?: (item: Subscription) => void,
  onRemoved?: (id: string) => void,
  onDeletedClose?: () => void,
) {
  const [request, setRequest] = useState<{
    action: Action;
    items: Subscription[];
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const completed = useRef(new Set<string>());
  const locked = useRef(false);
  const [done, setDone] = useState(false);
  const close = () => {
    if (locked.current) return;
    const deleted = request?.action === "delete" && completed.current.size > 0;
    setRequest(null);
    if (deleted) onDeletedClose?.();
  };
  const run = async () => {
    if (!request || locked.current || done) return;
    locked.current = true;
    setBusy(true);
    setMessage("");
    const errors: string[] = [];
    try {
      for (const item of request.items) {
        if (completed.current.has(item.id)) continue;
        try {
          if (request.action === "delete") {
            await client.deleteSubscription(item.id);
            onRemoved?.(item.id);
          } else {
            const changed =
              request.action === "archive"
                ? await client.archiveSubscription(item.id)
                : await client.restoreSubscription(item.id);
            onChanged?.(changed);
          }
          completed.current.add(item.id);
        } catch (error) {
          errors.push(
            `${item.name}: ${error instanceof Error ? error.message : "Request failed"}`,
          );
        }
      }
      setDone(errors.length === 0);
      setMessage(
        errors.length
          ? errors.join(" · ")
          : `${labels[request.action]} complete. Articles preserved.`,
      );
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  return {
    open(action: Action, items: Subscription[]) {
      if (!items.length || locked.current) return;
      completed.current = new Set();
      setDone(false);
      setMessage("");
      setRequest({ action, items: [...items] });
    },
    dialog: request && (
      <ModalDialog
        title={`${labels[request.action]} ${request.items.length === 1 ? "subscription" : "subscriptions"}`}
        onClose={close}
      >
        <div class="subscription-lifecycle-summary">
          <strong>
            {request.items.length === 1
              ? request.items[0].name
              : `${request.items.length} subscriptions`}
          </strong>
          <p>
            {request.action === "delete"
              ? "Remove from all subscription lists, including Archived. Existing articles and reading state stay in your feed. This subscription cannot be restored."
              : request.action === "archive"
                ? "Stop collecting new articles and move to Archived. Existing articles stay in your feed. You can restore the subscription later."
                : "Return to the active list and resume collecting articles."}
          </p>
        </div>
        <StatusRegion class="subscription-lifecycle-status" busy={busy}>
          {message}
        </StatusRegion>
        <div class="modal-actions">
          <button class="secondary-button" disabled={busy} onClick={close}>
            Close
          </button>
          <button
            class={
              request.action === "delete" ? "danger-button" : "primary-button"
            }
            disabled={busy || done}
            aria-busy={busy}
            onClick={run}
          >
            <span style={{ opacity: busy ? 0 : undefined }}>
              Confirm {request.action}
            </span>
            {busy && (
              <span class="async-button__pending">
                <span class="spinner" />
              </span>
            )}
          </button>
        </div>
      </ModalDialog>
    ),
  };
}
