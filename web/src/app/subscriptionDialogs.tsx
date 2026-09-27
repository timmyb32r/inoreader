import { useState } from "preact/hooks";
import { type ApiClient } from "../api/client";
import {
  AutofillResistantField,
  AutofillResistantTextarea,
} from "../ui/fields";
import { Icon } from "../ui/Icon";
import { ModalDialog } from "../ui/ModalDialog";
import type { Subscription } from "../api/viewModels";
export function AddSubscription({
  client,
  workspaceId,
  onClose,
  onWebFeed,
  onDone,
}: {
  client: ApiClient;
  workspaceId: string;
  onClose: () => void;
  onWebFeed: () => void;
  onDone: (item: Subscription) => void;
}) {
  const [url, setUrl] = useState("");
  const [stage, setStage] = useState<"entry" | "preview">("entry");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<{
    title: string;
    kind: string;
    articles: { title: string }[];
  } | null>(null);
  const discover = (event: Event) => {
    event.preventDefault();
    if (!url || pending) return;
    const requestedUrl = url;
    setPending(true);
    setError("");
    client
      .discoverFeed(url)
      .then((value) => {
        if (url === requestedUrl) {
          setPreview(value);
          setStage("preview");
        }
      })
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setPending(false));
  };
  const add = () => {
    if (pending) return;
    setPending(true);
    setError("");
    client
      .addSubscription(workspaceId, url, preview?.title)
      .then(onDone)
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setPending(false));
  };
  return (
    <ModalDialog
      title="Add a subscription"
      description="Paste a feed or public website URL. Nothing is added until you confirm."
      onClose={onClose}
    >
      {stage === "entry" ? (
        <form onSubmit={discover}>
          <div class="modal__body">
            <label>
              Feed or website URL
              <AutofillResistantField
                type="url"
                value={url}
                onInput={(e) => setUrl(e.currentTarget.value)}
                placeholder="https://example.com/feed.xml"
                required
                autoFocus
              />
            </label>
            <span class="field-help field-help--error" role="alert">
              {error}
            </span>
            <div class="scope-note">
              <Icon name="globe" />
              <span>
                <strong>Safe discovery</strong>
                <small>
                  Redirects and every network hop are checked before connecting.
                </small>
              </span>
            </div>
          </div>
          <footer class="modal__actions">
            <button type="button" class="secondary-button" onClick={onWebFeed}>
              Build a Web feed
            </button>
            <span />
            <button type="button" class="secondary-button" onClick={onClose}>
              Cancel
            </button>
            <button class="primary-button" disabled={pending || !url}>
              {pending ? (
                <>
                  <span class="spinner" />
                  Checking…
                </>
              ) : (
                "Check URL"
              )}
            </button>
          </footer>
        </form>
      ) : (
        <>
          <div class="modal__body">
            <div class="feed-preview">
              <span class="source__mark source__mark--rust">
                {preview?.title.slice(0, 1) ?? "F"}
              </span>
              <div>
                <h3>{preview?.title ?? "Discovered feed"}</h3>
                <p>
                  {preview?.kind.replaceAll("_", " ")} · {url}
                </p>
              </div>
              <span class="success-badge">
                <Icon name="check" size={14} />
                Feed found
              </span>
            </div>
            {preview?.articles.length ? (
              <div class="preview-articles">
                <small>AVAILABLE INITIAL ITEMS</small>
                {preview.articles.slice(0, 5).map((item, index) => (
                  <div key={`${item.title}-${index}`}>
                    <strong>{item.title}</strong>
                  </div>
                ))}
              </div>
            ) : null}
            <span class="field-help field-help--error" role="alert">
              {error}
            </span>
          </div>
          <footer class="modal__actions">
            <button
              class="secondary-button"
              disabled={pending}
              onClick={() => setStage("entry")}
            >
              Back
            </button>
            <span />
            <button
              class="secondary-button"
              disabled={pending}
              onClick={onClose}
            >
              Cancel
            </button>
            <button class="primary-button" disabled={pending} onClick={add}>
              {pending ? (
                <>
                  <span class="spinner" />
                  Adding…
                </>
              ) : (
                "Add subscription"
              )}
            </button>
          </footer>
        </>
      )}
    </ModalDialog>
  );
}
export function ReasonDialog({
  title,
  action,
  onClose,
  onDone,
}: {
  title: string;
  action: string;
  onClose: () => void;
  onDone: (reason: string) => Promise<void>;
}) {
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  const [touched, setTouched] = useState(false);
  const [error, setError] = useState("");
  const invalid = reason.trim().length === 0;
  const submit = (event: Event) => {
    event.preventDefault();
    setTouched(true);
    if (invalid) return;
    setPending(true);
    setError("");
    onDone(reason)
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setPending(false));
  };
  const close = () => {
    if (!pending) onClose();
  };
  return (
    <ModalDialog
      title={title}
      description={
        title.startsWith("Archive")
          ? "Updates stop for every subscription. Your library and individual pauses stay intact."
          : "New delivery stops for this subscription. Existing articles remain available."
      }
      onClose={close}
    >
      <form onSubmit={submit} aria-busy={pending}>
        <div class="modal__body">
          <label>
            Reason <span class="required">Required</span>
            <AutofillResistantTextarea
              disabled={pending}
              value={reason}
              onInput={(e) => setReason(e.currentTarget.value)}
              onBlur={() => setTouched(true)}
              rows={4}
              placeholder="Why are you pausing updates?"
              autoFocus
              aria-invalid={touched && invalid}
              aria-describedby="reason-help"
            />
            <span
              id="reason-help"
              class={`field-help ${touched && invalid ? "field-help--error" : ""}`}
            >
              {touched && invalid
                ? "Enter a reason before continuing."
                : error ||
                  "Saved exactly as entered, with your name and the current time."}
            </span>
          </label>
        </div>
        <footer class="modal__actions">
          <span />
          <span />
          <button
            type="button"
            class="secondary-button"
            disabled={pending}
            onClick={close}
          >
            Cancel
          </button>
          <button
            class="danger-button"
            disabled={pending || (touched && invalid)}
          >
            {pending ? (
              <>
                <span class="spinner" />
                Saving…
              </>
            ) : (
              action
            )}
          </button>
        </footer>
      </form>
    </ModalDialog>
  );
}
export function RestoreDialog({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: () => Promise<void>;
}) {
  const [pending, setPending] = useState(false),
    [error, setError] = useState("");
  const close = () => {
    if (!pending) onClose();
  };
  return (
    <ModalDialog
      title="Restore workspace"
      description="Subscriptions that were active before archiving will resume. Individually paused subscriptions remain paused."
      onClose={close}
    >
      <div class="modal__body">
        <div class="scope-note">
          <Icon name="refresh" />
          <span>
            <strong>Catch-up resumes from the saved boundary</strong>
            <small>
              Articles no longer available from their source may not be
              recovered.
            </small>
          </span>
        </div>
        <span class="field-help field-help--error" role="alert">
          {error}
        </span>
      </div>
      <footer class="modal__actions">
        <span />
        <span />
        <button class="secondary-button" disabled={pending} onClick={close}>
          Cancel
        </button>
        <button
          class="primary-button"
          disabled={pending}
          onClick={() => {
            if (pending) return;
            setPending(true);
            setError("");
            onDone()
              .catch((failure: Error) => setError(failure.message))
              .finally(() => setPending(false));
          }}
        >
          {pending ? (
            <>
              <span class="spinner" />
              Restoring…
            </>
          ) : (
            "Restore workspace"
          )}
        </button>
      </footer>
    </ModalDialog>
  );
}
