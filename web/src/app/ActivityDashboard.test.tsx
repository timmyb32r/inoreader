import { render, screen } from "@testing-library/preact";
import { ActivityDashboard, activityStorageKey, readActivity, recordActivity } from "./ActivityDashboard";

describe("activity dashboard", () => {
  const memory = new Map<string,string>();
  beforeEach(() => {
    memory.clear();
    Object.defineProperty(window, "localStorage", { configurable:true, value:{ getItem:(key:string)=>memory.get(key)??null, setItem:(key:string,value:string)=>memory.set(key,value), clear:()=>memory.clear() } });
  });

  it("persists elapsed time under an account-specific key", () => {
    recordActivity("account-one", 90_000, new Date("2026-09-26T12:00:00Z"));
    expect(readActivity("account-one")["2026-09-26"]).toBe(90_000);
    expect(readActivity("account-two")).toEqual({});
    expect(window.localStorage.getItem(activityStorageKey("account-one"))).not.toBeNull();
  });

  it("renders a fixed daily grid and accessible minute labels", () => {
    render(<ActivityDashboard activity={{ "2026-09-26": 3_600_000 }} workspaceName="Personal" onOpenLibrary={()=>undefined}/>);
    expect(screen.getByRole("heading", { name: "Your reading activity" })).toBeVisible();
    expect(screen.getByRole("grid", { name: "Daily minutes" }).querySelectorAll('[role="gridcell"]')).toHaveLength(364);
  });
});
