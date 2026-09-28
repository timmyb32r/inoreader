import { CountdownTimer } from "./timer/CountdownTimer";
import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { ArticleChatWidget } from "../ai/ArticleChatWidget";
import { DeepSeekProfile } from "../ai/DeepSeekProfile";
import { useAiProfile } from "../ai/useAiProfile";
import { useArticleChat } from "../ai/useArticleChat";
import { type ApiClient, type WebFeedRecipeView } from "../api/client";
import { GlossaryPanel } from "../glossary/GlossaryPanel";
import { useGlossary } from "../glossary/useGlossary";
import { reportLibraryReady } from "../performanceDiagnostics";
import { Icon } from "../ui/Icon";
import { ModalDialog } from "../ui/ModalDialog";
import { ActivityDashboard, useActivityTracker } from "./ActivityDashboard";
import { AdvancedSettings } from "./AdvancedSettings";
import { ArticleReader } from "./ArticleReader";
import { ArticleListPanel } from "./ArticleListPanel";
import type { Subscription, Workspace } from "../api/viewModels";
import { RulesDialog } from "./RulesDialog";
import {
  AddSubscription,
  ReasonDialog,
  RestoreDialog,
} from "./subscriptionDialogs";
import { ReaderSidebar } from "./ReaderSidebar";
import { SubscriptionsPage } from "./SubscriptionsPage";
import { usePopupNavigation } from "./usePopupNavigation";
import { useReaderController } from "./useReaderController";
import { useSubscriptionCommands } from "./useSubscriptionCommands";
import { useWorkspaceSwitch } from "./useWorkspaceSwitch";
import { WebFeedBuilder } from "./WebFeedBuilder";

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
  const modalRevision = useRef(0);
  const setModal = (next: Modal) => {
    modalRevision.current++;
    setModalStack((stack) =>
      next === null ? stack.slice(0, -1) : [...stack, next],
    );
  };
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
  const [editingRecipe, setEditingRecipe] = useState<WebFeedRecipeView | null>(
    null,
  );
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
  const subscriptionCommands = useSubscriptionCommands(
    client,
    account.id,
    workspaceId,
    (changed) =>
      setSubscriptions((items) =>
        items.map((item) => (item.id === changed.id ? changed : item)),
      ),
  );
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
    async () => {
      await Promise.all([
        reader.waitForWrites(),
        subscriptionCommands.waitForWrites(),
      ]);
    },
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
        <CountdownTimer accountId={bootstrap.account.id} />
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
          <ReaderSidebar
            key={`${account.id}/${workspaceId}`}
            {...{
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
              announce,
            }}
            onArchive={() => setModal("archive")}
            onAdd={() => setModal("add")}
            onSelectSubscription={(id) => {
              if (navigate("/reader")) {
                loadPage("subscription", id, undefined, undefined, 1);
                setMobilePanel("list");
              }
            }}
            onPause={(id) => {
              setPauseTarget(id);
              setModal("pause");
            }}
            onSettings={() => {
              setSettingsTarget(selectedSubscriptionId);
              setModal("shortcuts");
            }}
            onRefresh={subscriptionCommands.refresh}
          />
          {readerPath === "/" ? (
            <ActivityDashboard
              activity={activity}
              workspaceName={workspace}
              onOpenLibrary={() => chooseView("feed")}
            />
          ) : (
            <>
              <ArticleListPanel
                reader={reader}
                client={client}
                workspace={workspace}
                selectedSubscription={selectedSubscription}
                archived={archived}
                archiveReason={
                  workspaces.find((item) => item.id === workspaceId)
                    ?.archiveReason
                }
                mobilePanel={mobilePanel}
                newCount={newCount}
                navigate={navigate}
                announce={announce}
              />
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
                  onOpenSubscription={(id) =>
                    navigate(`/subscriptions/${encodeURIComponent(id)}`)
                  }
                  onUpdate={(patch) => update(selected.id, patch)}
                  onRefresh={() => reader.refreshFullText(selected.id)}
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
          onRefresh={async (id) => {
            await subscriptionCommands.refresh(id);
          }}
          onPause={(id) => {
            setPauseTarget(id);
            setModal("pause");
          }}
          onRemoved={(id) => {
            setSubscriptions((items) => items.filter((item) => item.id !== id));
            reader.forgetSubscription(id);
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
          onDone={async (reason) => {
            const generation = modalRevision.current;
            const id = pauseTarget ?? selectedSubscription!.id;
            const applied = await subscriptionCommands.pause(id, reason);
            if (applied && modalRevision.current === generation) {
              setPauseTarget(null);
              setModal(null);
              announce("Subscription paused");
            }
          }}
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
            onResume={async () => {
              if (settingsSubscription)
                await subscriptionCommands.resume(settingsSubscription.id);
            }}
          />
        </div>
      )}
    </div>
  );
}
