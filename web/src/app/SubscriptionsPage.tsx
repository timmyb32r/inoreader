import { useLayoutEffect, useRef } from "preact/hooks";
import type { ApiClient, WebFeedRecipeView } from "../api/client";
import { Icon } from "../ui/Icon";
import type { Subscription } from "./data";
import { SubscriptionCatalog } from "./SubscriptionCatalog";
import { SubscriptionDetails, type DetailTab } from "./SubscriptionDetails";
export function SubscriptionsPage({
  client,
  workspaceId,
  workspaceName,
  subscriptions,
  subscriptionId,
  initialTab,
  onBack,
  onOpenDetail,
  onOpenArticles,
  onRefresh,
  onPause,
  onChanged,
  onEditRecipe,
  onDirtyNoteChange,
}: {
  client: ApiClient;
  workspaceId: string;
  workspaceName: string;
  subscriptions: Subscription[];
  subscriptionId?: string;
  initialTab?: DetailTab;
  onBack: () => void;
  onOpenDetail?: (id: string) => void;
  onOpenArticles: (id: string, articleId?: string) => void;
  onRefresh: (id: string) => Promise<void>;
  onPause: (id: string) => void;
  onChanged?: (item: Subscription) => void;
  onEditRecipe?: (recipe: WebFeedRecipeView) => void;
  onDirtyNoteChange?: (dirty: boolean) => void;
}) {
  const overlay = useRef<HTMLDivElement>(null);
  const close = useRef(onBack);
  close.current = onBack;
  const catalogVisited = useRef(!subscriptionId);
  if (!subscriptionId) catalogVisited.current = true;
  useLayoutEffect(() => {
    const trigger =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    overlay.current
      ?.querySelector<HTMLButtonElement>(".subscriptions-window__close")
      ?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.key !== "Escape") return;
      const dialogs = [...document.querySelectorAll('[role="dialog"]')].filter(
        (item) => !item.closest("[hidden]"),
      );
      if (dialogs.at(-1) !== overlay.current) return;
      event.preventDefault();
      close.current();
    };
    window.addEventListener("keydown", keyboard);
    return () => {
      window.removeEventListener("keydown", keyboard);
      trigger?.focus();
    };
  }, []);
  return (
    <div
      ref={overlay}
      class="subscriptions-overlay"
      role="dialog"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onBack();
      }}
      aria-modal="true"
      aria-label={subscriptionId ? "Subscription details" : "Subscriptions"}
    >
      <div class="subscriptions-window">
        <button
          class="subscriptions-window__close"
          aria-label="Close subscriptions"
          onClick={onBack}
        >
          <Icon name="close" />
        </button>
        {subscriptionId && (
          <SubscriptionDetails
            key={`${subscriptionId}:${initialTab ?? "overview"}`}
            {...{
              client,
              workspaceId,
              workspaceName,
              subscriptions,
              subscriptionId,
              initialTab,
              onBack,
              onOpenArticles,
              onRefresh,
              onPause,
              onChanged,
              onEditRecipe,
              onDirtyNoteChange,
            }}
          />
        )}
        {catalogVisited.current && (
          <div hidden={!!subscriptionId}>
            <SubscriptionCatalog
              {...{
                client,
                workspaceId,
                workspaceName,
                subscriptions,
                onBack,
                onRefresh,
                onPause,
                onOpenDetail,
                onChanged,
              }}
            />
          </div>
        )}
      </div>
    </div>
  );
}
