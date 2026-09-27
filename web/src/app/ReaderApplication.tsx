import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { ArticleChatWidget } from "../ai/ArticleChatWidget";
import { DeepSeekProfile } from "../ai/DeepSeekProfile";
import { useAiProfile } from "../ai/useAiProfile";
import { useArticleChat } from "../ai/useArticleChat";
import { type ApiClient, type WebFeedRecipeView } from "../api/client";
import { GlossaryPanel } from "../glossary/GlossaryPanel";
import { useGlossary } from "../glossary/useGlossary";
import { reportLibraryReady } from "../performanceDiagnostics";
import { Icon, type IconName } from "../ui/Icon";
import { ModalDialog } from "../ui/ModalDialog";
import { ActivityDashboard, useActivityTracker } from "./ActivityDashboard";
import { AdvancedSettings } from "./AdvancedSettings";
import { ArticleReader } from "./ArticleReader";
import { ArticleRow, EmptyState } from "./ArticleRow";
import type { Subscription, Workspace } from "./data";
import { RulesDialog } from "./RulesDialog";
import {
  AddSubscription,
  ReasonDialog,
  RestoreDialog,
} from "./subscriptionDialogs";
import { SubscriptionIcon } from "./SubscriptionIcon";
import { SubscriptionsPage } from "./SubscriptionsPage";
import { usePopupNavigation } from "./usePopupNavigation";
import { useReaderController } from "./useReaderController";
import { useWorkspaceSwitch } from "./useWorkspaceSwitch";
import { WebFeedBuilder } from "./WebFeedBuilder";
import { WorkspacePicker } from "./WorkspacePicker";

import type { Bootstrap } from "../api/client";
import { type View } from "./readerLocation";
type Modal =
  | "add"
  | "pause"
  | "archive"
  | "rules"
  | "webfeed"
  | "shortcuts"
  | "profile"
  | null;
const views: { id: View; label: string; icon: IconName }[] = [
  { id: "feed", label: "Feed", icon: "inbox" },
  { id: "later", label: "Read later", icon: "later" },
];

export function ReaderApplication({
  client,
  bootstrap,
  onSignOut,
}: {
  client: ApiClient;
  bootstrap: Bootstrap;
  onSignOut: () => void;
}) {
  const [theme, setTheme] = useState<"light" | "dark">("light");
  const [subscriptions, setSubscriptions] = useState<Subscription[]>(
    bootstrap.subscriptions,
  );
  const [workspaces, setWorkspaces] = useState<Workspace[]>(
    bootstrap.workspaces,
  );
  const [modalStack, setModalStack] = useState<Modal[]>([]);
  const modal = modalStack.at(-1) ?? null;
  const setModal = (next: Modal) =>
    setModalStack((stack) =>
      next === null ? stack.slice(0, -1) : [...stack, next],
    );
  const [pauseTarget, setPauseTarget] = useState<string | null>(null);
  const [settingsTarget, setSettingsTarget] = useState<string | null>(null);
  const [mobilePanel, setMobilePanel] = useState<"nav" | "list" | "article">(
    "list",
  );
  const [newCount, setNewCount] = useState(0);
  const [archived, setArchived] = useState(
    bootstrap.workspaces.find((w) => w.id === bootstrap.activeWorkspaceId)
      ?.archived ?? false,
  );
  const [notice, setNotice] = useState("");
  const [signingOut, setSigningOut] = useState(false);
  const [accountMenuOpen, setAccountMenuOpen] = useState(false);
  const account = bootstrap.account;
  const aiProfileState = useAiProfile(client.ai, account.id);
  const aiProfile = aiProfileState.profile;
  const chat = useArticleChat(client.ai, account.id, aiProfile);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [refreshingSubscription, setRefreshingSubscription] = useState(false);
  const [editingRecipe, setEditingRecipe] = useState<WebFeedRecipeView | null>(
    null,
  );
  const [sourceMenu, setSourceMenu] = useState<string | null>(null);
  const noticeTimer = useRef<number>();
  const accountMenuRef = useRef<HTMLDivElement>(null);
  const dirtySubscriptionNote = useRef(false);
  const confirmDiscard = () =>
    !dirtySubscriptionNote.current ||
    window.confirm("Discard unsaved changes to your personal note?");
  const {
    path: locationPath,
    backgroundPath,
    navigate,
    close: closeSubscriptions,
  } = usePopupNavigation(confirmDiscard);
  const readerPath = locationPath.startsWith("/subscriptions")
    ? backgroundPath
    : locationPath;
  const announce = (message: string) => {
    setNotice(message);
    if (noticeTimer.current) window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(""), 1600);
  };
  const reader = useReaderController(
    client,
    bootstrap,
    subscriptions,
    announce,
    () => setMobilePanel("article"),
  );
  const {
    workspaceId,
    view,
    articles,
    selectedId,
    selectedSubscriptionId,
    filtered,
    selected,
    filteredRef,
    selectedRef,
    selectedIdRef,
    pageTotal,
    unreadTotal,
    newerCursor,
    olderCursor,
    pageNumber,
    paging,
    markingAll,
    pendingArticleMutations,
    open,
    update,
    loadPage,
    markAllRead,
    replaceWorkspace,
  } = reader;
  const glossary = useGlossary(client.glossary, account.id, workspaceId);
  const workspace =
    workspaces.find((item) => item.id === workspaceId)?.name ?? "Workspace";
  const selectedSubscription =
    subscriptions.find((item) => item.id === selectedSubscriptionId) ?? null;
  const settingsSubscription =
    subscriptions.find((item) => item.id === settingsTarget) ?? null;
  const activity = useActivityTracker(account.id);
  useEffect(() => {
    if (account.id)
      setSidebarCollapsed(
        window.localStorage?.getItem(
          `reader.sidebar.${account.id}.collapsed`,
        ) === "true",
      );
  }, [account.id]);
  useEffect(() => {
    reportLibraryReady();
    return () => {
      if (noticeTimer.current) clearTimeout(noticeTimer.current);
    };
  }, []);
  useEffect(() => {
    if (!accountMenuOpen) return;
    const closeOutside = (event: PointerEvent) => {
      if (!accountMenuRef.current?.contains(event.target as Node))
        setAccountMenuOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAccountMenuOpen(false);
    };
    window.addEventListener("pointerdown", closeOutside);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("pointerdown", closeOutside);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [accountMenuOpen]);

  const chooseView = (next: View) => {
    navigate("/reader");
    loadPage(next, null, undefined, undefined, 1);
    setMobilePanel("list");
  };

  const { switchingWorkspace, switchWorkspace } = useWorkspaceSwitch(
    client,
    workspaceId,
    (id, nextPage, nextSubscriptions) => {
      replaceWorkspace(id, nextPage);
      setSubscriptions(nextSubscriptions);
      setArchived(workspaces.find((item) => item.id === id)?.archived ?? false);
      setNewCount(0);
      setMobilePanel("list");
    },
    announce,
  );
  const goHome = () => {
    navigate("/");
  };
  const toggleSidebar = () =>
    setSidebarCollapsed((value) => {
      const next = !value;
      if (account.id)
        window.localStorage?.setItem(
          `reader.sidebar.${account.id}.collapsed`,
          String(next),
        );
      return next;
    });
  const subscriptionRoute =
    /^\/subscriptions(?:\/([^/]+)(?:\/(activity))?)?$/.exec(locationPath);
  const openRef = useRef(open),
    updateRef = useRef(update);
  openRef.current = open;
  updateRef.current = update;
  useLayoutEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (document.querySelector('[role="dialog"]')) return;
      if (
        event.target instanceof Element &&
        event.target.matches("input,textarea,select")
      )
        return;
      const current = filteredRef.current,
        currentSelected = selectedRef.current;
      const at = current.findIndex(
        (article) => article.id === selectedIdRef.current,
      );
      if (event.key === "j" && current[at + 1])
        openRef.current(current[at + 1].id);
      if (event.key === "k" && current[at - 1])
        openRef.current(current[at - 1].id);
      if (event.key === "l" && currentSelected)
        updateRef.current(currentSelected.id, {
          later: !currentSelected.later,
        });
      if (event.key === "Escape") setMobilePanel("list");
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, []);

  return (
    <div class={`app theme-${theme}`} data-theme={theme}>
      <header class="topbar">
        <button
          class="mobile-menu icon-button"
          aria-label="Open navigation"
          onClick={() => setMobilePanel("nav")}
        >
          <Icon name="menu" />
        </button>
        <a
          class="brand"
          href="/"
          aria-label="Reader home"
          onClick={(event) => {
            event.preventDefault();
            goHome();
          }}
        >
          <span class="brand__mark">
            <Icon name="feed" />
          </span>
          <span>Reader</span>
        </a>
        <div class="topbar__spacer" />
        <button
          class="search-stub"
          disabled
          aria-describedby="search-description"
        >
          <Icon name="search" />
          <span>Search</span>
          <kbd>Coming later</kbd>
        </button>
        <span id="search-description" class="sr-only">
          Search is not available in this version.
        </span>
        <button
          class="icon-button"
          aria-label={`Use ${theme === "light" ? "dark" : "light"} theme`}
          onClick={() => setTheme(theme === "light" ? "dark" : "light")}
        >
          <Icon name={theme === "light" ? "moon" : "sun"} />
        </button>
        <div class="account-menu" ref={accountMenuRef}>
          <button
            class="avatar"
            aria-label="Account menu"
            aria-haspopup="menu"
            aria-expanded={accountMenuOpen}
            onClick={() => setAccountMenuOpen((open) => !open)}
          >
            {account.initials}
          </button>
          {accountMenuOpen && (
            <div class="account-popover" role="menu">
              <div class="account-popover__identity">
                <strong>{account.displayName}</strong>
                <span>Administrator</span>
              </div>
              <button
                role="menuitem"
                onClick={() => {
                  setAccountMenuOpen(false);
                  setModal("profile");
                }}
              >
                Profile
              </button>
              <button
                role="menuitem"
                aria-busy={signingOut}
                disabled={signingOut}
                onClick={() => {
                  if (signingOut || !confirmDiscard()) return;
                  setSigningOut(true);
                  client
                    .signOut()
                    .then(onSignOut)
                    .catch((error: Error) => announce(error.message))
                    .finally(() => setSigningOut(false));
                }}
              >
                {signingOut ? (
                  <>
                    <span class="spinner" /> Signing out…
                  </>
                ) : (
                  "Sign out"
                )}
              </button>
            </div>
          )}
        </div>
      </header>
      <main
        class={`reader-grid${readerPath === "/" ? " reader-grid--home" : ""}${sidebarCollapsed ? " reader-grid--collapsed" : ""}`}
      >
        <>
          <aside
            class={`sidebar${sidebarCollapsed ? " sidebar--collapsed" : ""} panel-mobile-${mobilePanel === "nav" ? "show" : "hide"}`}
            aria-label="Reader navigation"
          >
            <button
              class="sidebar-toggle toolbar-tooltip"
              data-tooltip={
                sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
              }
              aria-label={
                sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
              }
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
              onArchive={() => setModal("archive")}
            />
            <nav class="nav-block" aria-label="Library">
              <button
                class={readerPath === "/" ? "nav-item active" : "nav-item"}
                onClick={goHome}
              >
                <Icon name="home" />
                <span>Home</span>
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
                class="nav-item nav-item--disabled"
                disabled
                title="Search is coming later"
              >
                <Icon name="search" />
                <span>Search</span>
                <small>Later</small>
              </button>
            </nav>
            <div class="sidebar__section-title">
              <button
                class="sidebar__section-link"
                onClick={() => navigate("/subscriptions")}
              >
                Subscriptions <em>{subscriptions.length}</em>
              </button>
              <button
                class="icon-button icon-button--small"
                aria-label="Add subscription"
                onClick={() => setModal("add")}
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
                      item.error
                        ? "source-entry source-entry--error"
                        : "source-entry"
                    }
                    key={item.id}
                  >
                    <button
                      disabled={paging}
                      class={
                        readerPath === "/reader" &&
                        selectedSubscriptionId === item.id
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
                        navigate("/reader");
                        loadPage(
                          "subscription",
                          item.id,
                          undefined,
                          undefined,
                          1,
                        );
                        setMobilePanel("list");
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
                      onClick={() =>
                        setSourceMenu(sourceMenu === item.id ? null : item.id)
                      }
                    >
                      <Icon name="dots" size={16} />
                    </button>
                    {sourceMenu === item.id && (
                      <div class="source-menu">
                        <button
                          onClick={() => {
                            setSourceMenu(null);
                            navigate(
                              `/subscriptions/${encodeURIComponent(item.id)}`,
                            );
                          }}
                        >
                          View details
                        </button>
                        <button
                          onClick={() => {
                            setSourceMenu(null);
                            client
                              .refreshSubscription(item.id)
                              .then(() => announce("Refresh queued"))
                              .catch((e: Error) => announce(e.message));
                          }}
                        >
                          Refresh
                        </button>
                        <button
                          onClick={() => {
                            setSourceMenu(null);
                            setPauseTarget(item.id);
                            setModal("pause");
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
                  setSettingsTarget(selectedSubscriptionId);
                  setModal("shortcuts");
                }}
              >
                <Icon name="settings" />
                <span>Settings</span>
              </button>
            </div>
          </aside>
          {readerPath === "/" ? (
            <ActivityDashboard
              activity={activity}
              workspaceName={workspace}
              onOpenLibrary={() => chooseView("feed")}
            />
          ) : (
            <>
              <section
                class={`article-list panel-mobile-${mobilePanel === "list" ? "show" : "hide"}`}
                aria-label="Article list"
              >
                <header class="list-header">
                  <div class="list-header__title">
                    {selectedSubscription ? (
                      <a
                        class="subscription-heading"
                        href={`/subscriptions/${encodeURIComponent(selectedSubscription.id)}`}
                        aria-label={`Open settings for ${selectedSubscription.name}`}
                        onClick={(event) => {
                          event.preventDefault();
                          navigate(
                            `/subscriptions/${encodeURIComponent(selectedSubscription.id)}`,
                          );
                        }}
                      >
                        <p class="eyebrow">Subscription</p>
                        <h1>{selectedSubscription.name}</h1>
                      </a>
                    ) : (
                      <>
                        <p class="eyebrow">{workspace}</p>
                        <h1>
                          {view === "feed"
                            ? `Feed (${unreadTotal})`
                            : views.find((v) => v.id === view)?.label}
                        </h1>
                      </>
                    )}
                  </div>
                  <div class="list-header__tools">
                    <button
                      class="icon-button"
                      aria-label="Refresh subscription"
                      aria-busy={refreshingSubscription}
                      disabled={!selectedSubscription || refreshingSubscription}
                      title={
                        selectedSubscription
                          ? "Refresh this subscription"
                          : "Choose a subscription to refresh"
                      }
                      onClick={() => {
                        if (!selectedSubscription || refreshingSubscription)
                          return;
                        setRefreshingSubscription(true);
                        client
                          .refreshSubscription(selectedSubscription.id)
                          .then(() => announce("Refresh queued"))
                          .catch((error: Error) => announce(error.message))
                          .finally(() => setRefreshingSubscription(false));
                      }}
                    >
                      {refreshingSubscription ? (
                        <span class="spinner" />
                      ) : (
                        <Icon name="refresh" />
                      )}
                    </button>
                  </div>
                </header>
                {archived && (
                  <div class="archive-strip">
                    <Icon name="archive" />
                    <span>
                      This workspace is archived.{" "}
                      {workspaces.find((item) => item.id === workspaceId)
                        ?.archiveReason
                        ? `Reason: ${workspaces.find((item) => item.id === workspaceId)?.archiveReason}. `
                        : ""}
                      Your library remains readable.
                    </span>
                  </div>
                )}
                {selectedSubscription?.status === "paused" && (
                  <div class="archive-strip">
                    <Icon name="pause" />
                    <span>
                      Paused
                      {selectedSubscription.reason
                        ? `: ${selectedSubscription.reason}`
                        : ""}
                      {selectedSubscription.reasonAt
                        ? ` · ${new Date(selectedSubscription.reasonAt).toLocaleString("en-US")}`
                        : ""}
                    </span>
                  </div>
                )}
                {newCount > 0 && (
                  <button
                    class="new-items"
                    disabled={paging}
                    aria-busy={paging}
                    onClick={() =>
                      loadPage(
                        view,
                        selectedSubscriptionId,
                        undefined,
                        undefined,
                        1,
                      )
                    }
                  >
                    <span>
                      {paging ? (
                        <>
                          <span class="spinner" />
                          Loading…
                        </>
                      ) : (
                        `${newCount} new articles`
                      )}
                    </span>
                    <span>Show now</span>
                  </button>
                )}
                <div class="list-controls">
                  <button
                    disabled={
                      markingAll ||
                      pendingArticleMutations.size > 0 ||
                      !filtered.some((article) => !article.read)
                    }
                    aria-busy={markingAll}
                    onClick={markAllRead}
                  >
                    {markingAll ? (
                      <span class="spinner" />
                    ) : (
                      <Icon name="check" size={15} />
                    )}{" "}
                    {markingAll ? "Marking…" : "Mark all read"}
                  </button>
                  <span>
                    {pageTotal
                      ? `${(pageNumber - 1) * 50 + 1}–${Math.min(pageNumber * 50, pageTotal)} of ${pageTotal}`
                      : "0 articles"}
                  </span>
                  <span>Newest first</span>
                </div>
                <div class="article-scroll">
                  {filtered.length ? (
                    filtered.map((article) => (
                      <ArticleRow
                        article={article}
                        selected={article.id === selected?.id}
                        laterPending={pendingArticleMutations.has(
                          `${article.id}:later`,
                        )}
                        onOpen={() => open(article.id)}
                        onLater={() =>
                          update(article.id, { later: !article.later })
                        }
                      />
                    ))
                  ) : (
                    <EmptyState />
                  )}
                </div>
                <nav class="article-pager" aria-label="Article pages">
                  <button
                    disabled={!newerCursor || paging}
                    aria-busy={paging}
                    onClick={() =>
                      newerCursor &&
                      loadPage(
                        view,
                        selectedSubscriptionId,
                        newerCursor,
                        "newer",
                        Math.max(1, pageNumber - 1),
                      )
                    }
                  >
                    ← Newer
                  </button>
                  <span>50 articles per request</span>
                  <button
                    disabled={!olderCursor || paging}
                    aria-busy={paging}
                    onClick={() =>
                      olderCursor &&
                      loadPage(
                        view,
                        selectedSubscriptionId,
                        olderCursor,
                        "older",
                        pageNumber + 1,
                      )
                    }
                  >
                    Older →
                  </button>
                </nav>
              </section>
              {selected ? (
                <ArticleReader
                  key={`${account.id}:${workspaceId}:${selected.id}`}
                  onDefinitions={() =>
                    void glossary.open(selected.id, selected.title)
                  }
                  definitionsPending={
                    glossary.busy && glossary.target?.id === selected.id
                  }
                  aiClient={client.ai}
                  workspaceId={workspaceId}
                  translationEnabled={!!aiProfile?.enabled}
                  article={selected}
                  pending={pendingArticleMutations}
                  className={`panel-mobile-${mobilePanel === "article" ? "show" : "hide"}`}
                  onBack={() => setMobilePanel("list")}
                  onUpdate={(patch) => update(selected.id, patch)}
                  onRefresh={() =>
                    client.refreshFullText(workspaceId, selected.id)
                  }
                  onNotice={announce}
                  summaryPending={!!chat.busy}
                  onSummarize={() =>
                    void chat.open({
                      articleId: selected.id,
                      workspaceId,
                      title: selected.title,
                    })
                  }
                />
              ) : (
                <section class="article-reader empty-reader">
                  <Icon name="inbox" size={28} />
                  <p>Select an article to read</p>
                </section>
              )}
            </>
          )}
        </>
      </main>
      {subscriptionRoute && (
        <SubscriptionsPage
          client={client}
          workspaceId={workspaceId}
          workspaceName={workspace}
          subscriptions={subscriptions}
          articles={articles}
          subscriptionId={
            subscriptionRoute[1]
              ? decodeURIComponent(subscriptionRoute[1])
              : undefined
          }
          initialTab={
            subscriptionRoute[2] === "activity" ? "activity" : "overview"
          }
          onBack={closeSubscriptions}
          onOpenDetail={(id) =>
            navigate(`/subscriptions/${encodeURIComponent(id)}`)
          }
          onOpenArticles={(id, articleId) => {
            if (navigate("/reader")) {
              loadPage("subscription", id, undefined, undefined, 1, articleId);
              if (!articleId) setMobilePanel("list");
            }
          }}
          onRefresh={(id) => client.refreshSubscription(id)}
          onPause={(id) => {
            setPauseTarget(id);
            setModal("pause");
          }}
          onChanged={(changed) =>
            setSubscriptions((items) =>
              items.map((item) => (item.id === changed.id ? changed : item)),
            )
          }
          onEditRecipe={(recipe) => {
            setEditingRecipe(recipe);
            setModal("webfeed");
          }}
          onDirtyNoteChange={(dirty) => {
            dirtySubscriptionNote.current = dirty;
          }}
        />
      )}
      <div class="live-region" aria-live="polite">
        {notice}
      </div>
      {modalStack.includes("add") && (
        <div hidden={modal !== "add"}>
          <AddSubscription
            client={client}
            workspaceId={workspaceId}
            onClose={() => setModal(null)}
            onWebFeed={() => {
              setEditingRecipe(null);
              setModal("webfeed");
            }}
            onDone={(added) => {
              setSubscriptions((items) => [...items, added]);
              setModal(null);
              announce("Subscription added");
            }}
          />
        </div>
      )}
      {modal === "pause" && (pauseTarget || selectedSubscription) && (
        <ReasonDialog
          title="Pause subscription"
          action="Pause subscription"
          onClose={() => {
            setPauseTarget(null);
            setModal(null);
          }}
          onDone={(reason) =>
            client
              .pauseSubscription(
                pauseTarget ?? selectedSubscription!.id,
                reason,
              )
              .then(() => {
                setSubscriptions((items) =>
                  items.map((item) =>
                    item.id === (pauseTarget ?? selectedSubscription!.id)
                      ? {
                          ...item,
                          status: "paused",
                          reason,
                          reasonAt: new Date().toISOString(),
                        }
                      : item,
                  ),
                );
                setPauseTarget(null);
                setModal(null);
                announce("Subscription paused");
              })
          }
        />
      )}
      {modal === "archive" && !archived && (
        <ReasonDialog
          title="Archive workspace"
          action="Archive workspace"
          onClose={() => setModal(null)}
          onDone={(reason) =>
            client.archiveWorkspace(workspaceId, reason).then(() => {
              setArchived(true);
              setWorkspaces((items) =>
                items.map((item) =>
                  item.id === workspaceId
                    ? {
                        ...item,
                        archived: true,
                        archiveReason: reason,
                        archiveReasonAt: new Date().toISOString(),
                      }
                    : item,
                ),
              );
              setModal(null);
              announce("Workspace archived");
            })
          }
        />
      )}
      {modal === "archive" && archived && (
        <RestoreDialog
          onClose={() => setModal(null)}
          onDone={() =>
            client.restoreWorkspace(workspaceId).then(() => {
              setArchived(false);
              setWorkspaces((items) =>
                items.map((item) =>
                  item.id === workspaceId ? { ...item, archived: false } : item,
                ),
              );
              setModal(null);
              announce("Workspace restored");
            })
          }
        />
      )}
      {modal === "rules" && selectedSubscription && (
        <RulesDialog
          client={client}
          workspaceId={workspaceId}
          subscriptionId={selectedSubscription.id}
          onClose={() => setModal(null)}
        />
      )}
      {modal === "webfeed" && (
        <WebFeedBuilder
          client={client}
          workspaceId={workspaceId}
          editing={editingRecipe}
          onClose={() => {
            setEditingRecipe(null);
            setModal(null);
          }}
          onDone={(added) => {
            setSubscriptions((items) => [...items, added]);
            setEditingRecipe(null);
            setModal(null);
            announce("Web feed created");
          }}
          onUpdated={() => {
            setEditingRecipe(null);
            setModal(null);
            announce("Web feed recipe updated; collection queued");
          }}
        />
      )}
      {glossary.target && (
        <GlossaryPanel
          key={`${account.id}:${workspaceId}:${glossary.target.id}`}
          controller={glossary}
        />
      )}
      {chat.visible && (
        <ArticleChatWidget controller={chat} profile={aiProfile} />
      )}
      {modal === "profile" && (
        <ModalDialog
          title="Profile"
          description="Your personal DeepSeek key and balance."
          onClose={() => setModal(null)}
          width="540px"
        >
          <DeepSeekProfile
            client={client.ai}
            onProfile={(value) => aiProfileState.update(account.id, value)}
            completedGeneration={chat.completedGeneration}
          />
          <footer class="modal__actions">
            <span />
            <span />
            <span />
            <button class="primary-button" onClick={() => setModal(null)}>
              Done
            </button>
          </footer>
        </ModalDialog>
      )}
      {modalStack.includes("shortcuts") && (
        <div hidden={modal !== "shortcuts"}>
          <AdvancedSettings
            onProfile={() => setModal("profile")}
            client={client}
            workspaceId={workspaceId}
            workspaceName={workspace}
            subscriptions={subscriptions}
            selectedSubscription={settingsSubscription}
            onSelectSubscription={setSettingsTarget}
            onSubscription={(changed) =>
              setSubscriptions((items) =>
                items.map((item) => (item.id === changed.id ? changed : item)),
              )
            }
            onWorkspace={(item) => {
              setWorkspaces((items) => [...items, item]);
              replaceWorkspace(item.id, {
                articles: [],
                total: 0,
                unreadTotal: 0,
              });
              setSubscriptions([]);
            }}
            onRename={(item) =>
              setWorkspaces((items) =>
                items.map((old) => (old.id === item.id ? item : old)),
              )
            }
            onEditWebFeed={(recipe) => {
              setEditingRecipe(recipe);
              setModal("webfeed");
            }}
            onClose={() => setModal(null)}
            onPause={() => {
              setPauseTarget(settingsTarget);
              setModal("pause");
            }}
            onResume={() => {
              if (!settingsSubscription) return;
              client.resumeSubscription(settingsSubscription.id).then(() => {
                setSubscriptions((items) =>
                  items.map((item) =>
                    item.id === settingsSubscription.id
                      ? { ...item, status: "active", reason: undefined }
                      : item,
                  ),
                );
                setModal(null);
                announce("Subscription resumed; catch-up queued");
              });
            }}
          />
        </div>
      )}
    </div>
  );
}
