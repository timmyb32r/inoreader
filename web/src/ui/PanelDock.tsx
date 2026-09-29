import { createContext, type ComponentChildren } from "preact";
import { useContext, useState } from "preact/hooks";
import { useFloatingPanel } from "./useFloatingPanel";
import "./panel-dock.css";

const Dock = createContext<
  ReturnType<typeof useFloatingPanel>["handle"] | null
>(null);
export const usePanelDock = () => useContext(Dock);

/** Both panes keep their component identity when docking or undocking. */
export function PanelDock({
  active,
  summary,
  terms,
}: {
  active: boolean;
  summary: ComponentChildren;
  terms: ComponentChildren;
}) {
  const floating = useFloatingPanel(!active, active);
  const [tab, setTab] = useState<"summary" | "terms">("terms");
  return (
    <Dock.Provider value={active ? floating.handle : null}>
      <div
        ref={floating.element}
        style={active ? floating.style : undefined}
        class={active ? "panel-dock" : "panel-dock--inactive"}
        data-tab={tab}
      >
        {active && (
          <nav class="panel-dock-tabs" aria-label="Article assistant panes">
            <button
              aria-pressed={tab === "summary"}
              onClick={() => setTab("summary")}
            >
              Summary
            </button>
            <button
              aria-pressed={tab === "terms"}
              onClick={() => setTab("terms")}
            >
              Terms
            </button>
          </nav>
        )}
        <div class="panel-dock-summary">{summary}</div>
        <div class="panel-dock-terms">{terms}</div>
      </div>
    </Dock.Provider>
  );
}
