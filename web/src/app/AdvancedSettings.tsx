import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient, WebFeedRecipeView } from "../api/client";
import { GlossarySettings } from "../glossary/GlossarySettings";
import {
  AutofillResistantField,
  AutofillResistantSelect,
  AutofillResistantTextarea,
} from "../ui/fields";
import { AsyncButton } from "../ui/AsyncButton";
import { Icon } from "../ui/Icon";
import { ModalDialog } from "../ui/ModalDialog";
import type { Subscription, Workspace } from "../api/viewModels";
export function AdvancedSettings({
  onProfile,
  client,
  workspaceId,
  workspaceName,
  subscriptions,
  selectedSubscription,
  onSelectSubscription,
  onSubscription,
  onWorkspace,
  onRename,
  onEditWebFeed,
  onClose,
  onPause,
  onResume,
}: {
  client: ApiClient;
  onProfile: () => void;
  workspaceId: string;
  workspaceName: string;
  subscriptions: Subscription[];
  selectedSubscription: Subscription | null;
  onSelectSubscription: (id: string | null) => void;
  onSubscription: (v: Subscription) => void;
  onWorkspace: (w: Workspace) => void;
  onRename: (w: Workspace) => void;
  onEditWebFeed: (recipe: WebFeedRecipeView) => void;
  onClose: () => void;
  onPause: () => void;
  onResume: () => Promise<void>;
}) {
  const [opml, setOpml] = useState(""),
    [previewId, setPreviewId] = useState(""),
    [name, setName] = useState(workspaceName),
    [subscriptionName, setSubscriptionName] = useState(
      selectedSubscription?.name ?? "",
    ),
    [username, setUsername] = useState(""),
    [pending, setPending] = useState(false),
    [confirmArchive, setConfirmArchive] = useState(false),
    [message, setMessage] = useState("");
  const pendingRef = useRef(false);
  const selection = useRef(selectedSubscription?.id);
  selection.current = selectedSubscription?.id;
  useEffect(() => {
    setSubscriptionName(selectedSubscription?.name ?? "");
    setConfirmArchive(false);
    setMessage("");
  }, [selectedSubscription?.id, selectedSubscription?.name]);
  const run = <T,>(work: () => Promise<T>, done: (value: T) => void) => {
    if (pendingRef.current) return;
    pendingRef.current = true;
    setPending(true);
    setMessage("");
    work()
      .then(done)
      .catch((e: Error) => setMessage(e.message))
      .finally(() =>
        window.setTimeout(() => {
          pendingRef.current = false;
          setPending(false);
        }, 250),
      );
  };
  const download = () =>
    run(
      () => client.exportOpml(workspaceId),
      (text) => {
        const url = URL.createObjectURL(
          new Blob([text], { type: "text/x-opml" }),
        );
        const link = document.createElement("a");
        link.href = url;
        link.download = `${workspaceName}.opml`;
        link.click();
        URL.revokeObjectURL(url);
        setMessage("Export downloaded");
      },
    );
  return (
    <ModalDialog
      title="Workspace & account"
      description="Manage this workspace, portable subscriptions and administrator links."
      onClose={onClose}
      width="680px"
    >
      <div class="modal__body settings-list">
        <section>
          <h3>DeepSeek</h3>
          <button class="settings-action" onClick={onProfile}>
            Open DeepSeek profile
          </button>
        </section>
        <GlossarySettings
          key={workspaceId}
          client={client.glossary}
          workspace={workspaceId}
        />
        <section>
          <h3>Workspace</h3>
          <label>
            Name
            <AutofillResistantField
              value={name}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </label>
          <button
            class="settings-action"
            disabled={pending || !name.trim()}
            onClick={() =>
              run(() => client.renameWorkspace(workspaceId, name), onRename)
            }
          >
            Rename current workspace
          </button>
          <button
            class="settings-action"
            disabled={pending || !name.trim()}
            onClick={() => run(() => client.createWorkspace(name), onWorkspace)}
          >
            <Icon name="plus" />
            Create as new workspace
          </button>
        </section>
        <section>
          <h3>Subscription</h3>
          <label>
            Choose subscription
            <AutofillResistantSelect
              value={selectedSubscription?.id ?? ""}
              onChange={(e) =>
                onSelectSubscription(e.currentTarget.value || null)
              }
            >
              <option value="">Not selected</option>
              {subscriptions.map((item) => (
                <option value={item.id} key={item.id}>
                  {item.name}
                  {item.status === "archived" ? " (archived)" : ""}
                </option>
              ))}
            </AutofillResistantSelect>
          </label>
          {selectedSubscription && (
            <>
              <label>
                Custom name
                <AutofillResistantField
                  value={subscriptionName}
                  onInput={(e) => setSubscriptionName(e.currentTarget.value)}
                />
              </label>
              <button
                class="settings-action"
                disabled={
                  pending ||
                  !subscriptionName ||
                  subscriptionName.trim() !== subscriptionName
                }
                onClick={() =>
                  run(
                    () =>
                      client.renameSubscription(
                        selectedSubscription.id,
                        subscriptionName,
                      ),
                    onSubscription,
                  )
                }
              >
                Rename subscription
              </button>
              {selectedSubscription.editableWebFeed && (
                <button
                  class="settings-action"
                  disabled={pending}
                  onClick={() =>
                    run(
                      () => client.getWebFeedRecipe(selectedSubscription.id),
                      onEditWebFeed,
                    )
                  }
                >
                  Edit Web feed recipe
                </button>
              )}
              {selectedSubscription.status === "archived" ? (
                <button
                  class="settings-action"
                  disabled={pending}
                  onClick={() =>
                    run(
                      () => client.restoreSubscription(selectedSubscription.id),
                      (value) => {
                        onSubscription(value);
                        setMessage(
                          "Subscription restored; catch-up queued. Items no longer available at the source may not be recovered.",
                        );
                      },
                    )
                  }
                >
                  <Icon name="refresh" />
                  Restore subscription
                </button>
              ) : (
                <>
                  {selectedSubscription.status === "paused" ? (
                    <AsyncButton
                      key={selectedSubscription.id}
                      class="settings-action"
                      disabled={pending}
                      onPress={async () => {
                        setMessage("");
                        await onResume();
                        if (selection.current === selectedSubscription.id)
                          setMessage("Subscription resumed; catch-up queued");
                      }}
                      onError={(error) =>
                        selection.current === selectedSubscription.id &&
                        setMessage(
                          error instanceof Error
                            ? error.message
                            : "Could not resume subscription",
                        )
                      }
                    >
                      <Icon name="refresh" />
                      Resume {selectedSubscription.name}
                    </AsyncButton>
                  ) : (
                    <button
                      class="settings-action"
                      disabled={pending}
                      onClick={onPause}
                    >
                      <Icon name="pause" />
                      Pause {selectedSubscription.name}
                    </button>
                  )}
                  <button
                    class="settings-action"
                    disabled={pending}
                    onClick={() => {
                      if (!confirmArchive) {
                        setConfirmArchive(true);
                        setMessage(
                          "Choose Archive again to confirm. Existing articles and full text stay in your library.",
                        );
                        return;
                      }
                      run(
                        () =>
                          client.archiveSubscription(selectedSubscription.id),
                        (value) => {
                          onSubscription(value);
                          setMessage(
                            "Archived. Existing articles and full text remain available.",
                          );
                        },
                      );
                    }}
                  >
                    <Icon name="archive" />
                    {confirmArchive ? "Confirm archive" : "Archive"}
                  </button>
                </>
              )}
            </>
          )}
        </section>
        <section>
          <h3>OPML</h3>
          <label>
            OPML document
            <AutofillResistantTextarea
              rows={3}
              value={opml}
              onInput={(e) => {
                setOpml(e.currentTarget.value);
                setPreviewId("");
              }}
            />
          </label>
          <button
            class="settings-action"
            disabled={pending || !opml}
            onClick={() =>
              run(
                () => client.importOpml(workspaceId, opml),
                (result) => {
                  setPreviewId(result.preview_id);
                  setMessage(
                    `${result.subscriptions} subscriptions ready${result.warnings.length ? `; ${result.warnings.join("; ")}` : ""}`,
                  );
                },
              )
            }
          >
            Preview import
          </button>
          <button
            class="settings-action"
            disabled={pending || !previewId}
            onClick={() =>
              run(
                () => client.importOpml(workspaceId, opml, previewId),
                () => {
                  setPreviewId("");
                  setMessage("Import applied");
                },
              )
            }
          >
            Apply previewed import
          </button>
          <button class="settings-action" disabled={pending} onClick={download}>
            Download OPML export
          </button>
        </section>
        <section>
          <h3>Administrator links</h3>
          <label>
            Username
            <AutofillResistantField
              value={username}
              onInput={(e) => setUsername(e.currentTarget.value)}
            />
          </label>
          <button
            class="settings-action"
            disabled={pending || !username}
            onClick={() =>
              run(
                () => client.createInvite(username),
                (result) => setMessage(`Invitation: ${result.url}`),
              )
            }
          >
            Create invitation
          </button>
          <button
            class="settings-action"
            disabled={pending || !username}
            onClick={() =>
              run(
                () => client.createPasswordReset(username),
                (result) => setMessage(`Password reset: ${result.url}`),
              )
            }
          >
            Create password reset
          </button>
        </section>
        <span class="field-help" role="status">
          {pending ? "Working…" : message}
        </span>
      </div>
      <footer class="modal__actions">
        <span />
        <span />
        <span />
        <button class="primary-button" onClick={onClose}>
          Done
        </button>
      </footer>
    </ModalDialog>
  );
}
