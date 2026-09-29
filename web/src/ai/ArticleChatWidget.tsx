import { usePanelDock } from "../ui/PanelDock";
import { useLayoutEffect } from "preact/hooks";
import type { AiProfile } from "../api/ai";
import { FloatingPanel } from "../ui/FloatingPanel";
import { Icon } from "../ui/Icon";
import { useFloatingPanel } from "../ui/useFloatingPanel";
import { ArticleChatContent } from "./ArticleChatContent";
import type { ArticleChatController } from "./useArticleChat";
import "./deepseek.css";
export function ArticleChatWidget({
  controller,
  profile,
}: {
  controller: ArticleChatController;
  profile: AiProfile | null;
}) {
  const { chat, target, collapsed } = controller;
  const dock = usePanelDock();
  const floating = useFloatingPanel(collapsed);
  useLayoutEffect(() => {
    if (!collapsed)
      floating.element.current
        ?.querySelector<HTMLElement>("textarea:not(:disabled)")
        ?.focus();
  }, [collapsed]);
  return (
    <FloatingPanel
      position={floating}
      class={`ai-chat${collapsed ? " ai-chat--collapsed" : ""}`}
      label={`Article chat: ${chat?.title ?? target?.title ?? "DeepSeek"}`}
      onClose={controller.close}
    >
      <header class="ai-chat__header">
        <div
          class="ai-chat__handle"
          role="button"
          tabIndex={0}
          aria-label="Move chat with arrow keys or drag"
          {...(dock ?? floating.handle)}
        >
          <small>DeepSeek</small>
          <strong title={chat?.title ?? target?.title}>
            {chat?.title ?? target?.title}
          </strong>
        </div>
        <button
          class="icon-button"
          aria-label={collapsed ? "Expand chat" : "Minimize chat"}
          title={collapsed ? "Expand" : "Minimize"}
          aria-expanded={!collapsed}
          onClick={() => controller.setCollapsed(!collapsed)}
        >
          <span aria-hidden="true">{collapsed ? "□" : "−"}</span>
        </button>
        <button
          class="icon-button"
          aria-label="Close chat"
          title="Close"
          onClick={controller.close}
        >
          <Icon name="close" />
        </button>
      </header>
      <ArticleChatContent
        controller={controller}
        profile={profile}
        collapsed={collapsed}
      />
    </FloatingPanel>
  );
}
