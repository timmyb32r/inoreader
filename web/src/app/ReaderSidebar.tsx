import { useState } from "preact/hooks";
import type { Subscription, Workspace } from "../api/viewModels";
import { Icon, type IconName } from "../ui/Icon";
import { AsyncButton } from "../ui/AsyncButton";
import { WorkspacePicker } from "./WorkspacePicker";
import { SubscriptionIcon } from "./SubscriptionIcon";
import type { View } from "./readerLocation";

const views: { id: View; label: string; icon: IconName }[] = [
  { id: "feed", label: "Feed", icon: "inbox" },
  { id: "later", label: "Read later", icon: "later" },
];
interface Props {
  sidebarCollapsed: boolean;
  mobilePanel: "nav" | "list" | "article";
  toggleSidebar: () => void;
  workspace: string;
  workspaces: Workspace[];
  archived: boolean;
  switchingWorkspace: boolean;
  switchWorkspace: (id: string) => Promise<void>;
  readerPath: string;
  goHome: () => void;
  paging: boolean;
  view: View;
  selectedSubscriptionId: string | null;
  chooseView: (view: View) => void;
  unreadTotal: number;
  subscriptions: Subscription[];
  navigate: (path: string) => boolean;
  onArchive: () => void;
  onAdd: () => void;
  onSelectSubscription: (id: string) => void;
  onPause: (id: string) => void;
  onSettings: () => void;
  onRefresh: (id: string) => Promise<unknown>;
  announce: (message: string) => void;
}
/** Navigation owns its local menus; application commands and routes are explicit inputs. */
export function ReaderSidebar({
  sidebarCollapsed,
  mobilePanel,
  toggleSidebar,
  workspace,
  workspaces,
  archived,
  switchingWorkspace,
  switchWorkspace,
  readerPath,
  goHome,
  paging,
  view,
  selectedSubscriptionId,
  chooseView,
  unreadTotal,
  subscriptions,
  navigate,
  onArchive,
  onAdd,
  onSelectSubscription,
  onPause,
  onSettings,
  onRefresh,
  announce,
}: Props) {
  const [sourceMenu, setSourceMenu] = useState<string | null>(null);
  const [sourceError, setSourceError] = useState<{
    id: string;
    message: string;
  } | null>(null);
  return (
    <aside
      class={`sidebar${sidebarCollapsed ? " sidebar--collapsed" : ""} panel-mobile-${mobilePanel === "nav" ? "show" : "hide"}`}
      aria-label="Reader navigation"
    >
      <button
        class="sidebar-toggle toolbar-tooltip"
        data-tooltip={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        aria-expanded={!sidebarCollapsed}
        onClick={toggleSidebar}
      >
        <span class="sidebar-toggle__arrow" aria-hidden="true" />
      </button>
      <WorkspacePicker
        value={workspace}
        workspaces={workspaces}
        archived={archived}
        pending={switchingWorkspace}
        onChange={switchWorkspace}
        onArchive={() => onArchive()}
      />
      <nav class="nav-block" aria-label="Library">
        <button
          class={readerPath === "/" ? "nav-item active" : "nav-item"}
          onClick={goHome}
        >
          <Icon name="home" />
          <span>Home</span>
        </button>
        <button
          class={
            readerPath.startsWith("/wiki") ? "nav-item active" : "nav-item"
          }
          aria-label="Wiki"
          title="Wiki"
          onClick={() => navigate("/wiki")}
        >
          <Icon name="book" />
          <span>Wiki</span>
        </button>
        {views.map((item) => (
          <button
            key={item.id}
            disabled={paging}
            class={
              readerPath === "/reader" &&
              view === item.id &&
              !selectedSubscriptionId
                ? "nav-item active"
                : "nav-item"
            }
            onClick={() => chooseView(item.id)}
          >
            <Icon name={item.icon} />
            <span>
              {item.id === "feed" ? `Feed (${unreadTotal})` : item.label}
            </span>
          </button>
        ))}
        <button
          class={`nav-item${readerPath === "/search" ? " active" : ""}`}
          onClick={() => navigate("/search")}
          title="Search"
        >
          <Icon name="search" />
          <span>Search</span>
        </button>
      </nav>
      <div class="sidebar__section-title">
        <button
          class="sidebar__section-link"
          aria-label={`Subscriptions ${subscriptions.length}`}
          title={
            sidebarCollapsed
              ? `Subscriptions (${subscriptions.length})`
              : undefined
          }
          onClick={() => navigate("/subscriptions")}
        >
          <span>Subscriptions</span> <em>{subscriptions.length}</em>
        </button>
        <button
          class="icon-button icon-button--small"
          aria-label="Add subscription"
          onClick={() => onAdd()}
        >
          <Icon name="plus" size={16} />
        </button>
      </div>
      <nav class="source-list" aria-label="Subscriptions">
        {subscriptions
          .filter((item) => item.status !== "archived")
          .map((item) => (
            <div
              class={
                item.error ? "source-entry source-entry--error" : "source-entry"
              }
              key={item.id}
            >
              <button
                disabled={paging}
                class={
                  readerPath === "/reader" && selectedSubscriptionId === item.id
                    ? "source active"
                    : "source"
                }
                title={
                  item.error ??
                  item.continuation ??
                  (item.lastUpdate
                    ? `Last successful update: ${item.lastUpdate}`
                    : "Waiting for the first successful update")
                }
                onClick={() => {
                  onSelectSubscription(item.id);
                }}
              >
                <SubscriptionIcon
                  name={item.name}
                  iconDataUrl={item.iconDataUrl}
                />
                <span class="source__copy">
                  <span class="source__name">{item.name}</span>
                  {item.error ? (
                    <small class="source__status source__status--error">
                      Update failed · {item.error}
                    </small>
                  ) : item.incomplete ? (
                    <small class="source__status">
                      Incomplete · continuation queued
                    </small>
                  ) : item.lastUpdate ? (
                    <small class="source__status">
                      Updated {item.lastUpdate}
                    </small>
                  ) : (
                    <small class="source__status">
                      Waiting for first update
                    </small>
                  )}
                </span>
                {item.status === "paused" ? (
                  <Icon name="pause" size={14} />
                ) : (
                  <em>{item.count}</em>
                )}
              </button>
              {item.error && (
                <button
                  class="source-error-log"
                  aria-label={`Open update log for ${item.name}`}
                  title="Open update log"
                  onClick={() =>
                    navigate(
                      `/subscriptions/${encodeURIComponent(item.id)}/activity`,
                    )
                  }
                >
                  Log
                </button>
              )}
              <button
                class="source-more"
                aria-label="More options"
                title={`More options for ${item.name}`}
                aria-expanded={sourceMenu === item.id}
                onClick={() => {
                  setSourceError(null);
                  setSourceMenu(sourceMenu === item.id ? null : item.id);
                }}
              >
                <Icon name="dots" size={16} />
              </button>
              {sourceMenu === item.id && (
                <div class="source-menu">
                  <button
                    onClick={() => {
                      setSourceMenu(null);
                      navigate(`/subscriptions/${encodeURIComponent(item.id)}`);
                    }}
                  >
                    View details
                  </button>
                  <AsyncButton
                    onPress={async () => {
                      await onRefresh(item.id);
                      setSourceMenu((current) =>
                        current === item.id ? null : current,
                      );
                      announce("Refresh queued");
                    }}
                    onError={(error) => {
                      setSourceError({
                        id: item.id,
                        message:
                          error instanceof Error
                            ? error.message
                            : "Refresh failed",
                      });
                    }}
                  >
                    Refresh
                  </AsyncButton>
                  <div class="source-menu__status" role="status">
                    {sourceError?.id === item.id ? sourceError.message : ""}
                  </div>
                  <button
                    onClick={() => {
                      setSourceMenu(null);
                      onPause(item.id);
                    }}
                  >
                    Pause
                  </button>
                </div>
              )}
            </div>
          ))}
      </nav>
      <div class="sidebar__footer">
        <button
          class="nav-item"
          aria-label="Settings"
          onClick={() => {
            onSettings();
          }}
        >
          <Icon name="settings" />
          <span>Settings</span>
        </button>
      </div>
    </aside>
  );
}
