import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";
import type { Article, Subscription, Workspace } from "./data";
import { ApiError, type ApiClient,type WebFeedRecipeView } from "../api/client";
import { Icon, type IconName } from "../ui/Icon";
import { AutofillResistantField, AutofillResistantSelect, AutofillResistantTextarea } from "../ui/fields";
import { Dialog } from "./Dialog";
import { AuthScreen } from "./AuthScreen";
import { WebFeedBuilder } from "./WebFeedBuilder";
import { RulesDialog } from "./RulesDialog";
import { SubscriptionsPage } from "./SubscriptionsPage";

type View = "all" | "unread" | "saved" | "later" | "trash";
type Modal = "add" | "pause" | "archive" | "rules" | "webfeed" | "shortcuts" | null;
const views: { id: View; label: string; icon: IconName }[] = [
  { id: "all", label: "All articles", icon: "inbox" }, { id: "unread", label: "Unread", icon: "unread" },
  { id: "saved", label: "Saved", icon: "star" }, { id: "later", label: "Read later", icon: "later" }, { id: "trash", label: "Trash", icon: "trash" },
];

export function App({ client }: { client: ApiClient }) {
  const [signedIn, setSignedIn] = useState<boolean | null>(null);
  const [theme, setTheme] = useState<"light" | "dark">("light");
  const [view, setView] = useState<View>("all");
  const [articles, setArticles] = useState<Article[]>([]);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [selectedSubscriptionId, setSelectedSubscriptionId] = useState<string | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [mobilePanel, setMobilePanel] = useState<"nav" | "list" | "article">("list");
  const [newCount, setNewCount] = useState(0);
  const [workspaceId, setWorkspaceId] = useState("");
  const [archived, setArchived] = useState(false);
  const [notice, setNotice] = useState("");
  const [loadError, setLoadError] = useState("");
  const [signingOut, setSigningOut] = useState(false);
  const [accountMenuOpen, setAccountMenuOpen] = useState(false);
  const [account, setAccount] = useState({ displayName: "", initials: "" });
  const [switchingWorkspace, setSwitchingWorkspace] = useState(false);
  const [markingAll, setMarkingAll] = useState(false);
  const [refreshingSubscription, setRefreshingSubscription] = useState(false);
  const [editingRecipe,setEditingRecipe]=useState<WebFeedRecipeView|null>(null);
  const [showingNew, setShowingNew] = useState(false);
  const [locationPath,setLocationPath]=useState(window.location.pathname);
  const [sourceMenu,setSourceMenu]=useState<string|null>(null);
  const [pendingArticleMutations, setPendingArticleMutations] = useState<Set<string>>(()=>new Set());
  const noticeTimer = useRef<number>();
  const workspaceGeneration = useRef(0);
  const articlesRef = useRef<Article[]>([]);
  const filteredRef = useRef<Article[]>([]);
  const selectedIdRef = useRef("");
  const selectedRef = useRef<Article>();
  const mutationGeneration = useRef(new Map<string, number>());
  const mutationQueue = useRef(new Map<string, Promise<void>>());
  const accountMenuRef = useRef<HTMLDivElement>(null);
  const dirtySubscriptionNote = useRef(false);
  const locationPathRef = useRef(window.location.pathname);
  const confirmDiscard = () => !dirtySubscriptionNote.current || window.confirm("Discard unsaved changes to your personal note?");
  const workspace = workspaces.find((item) => item.id === workspaceId)?.name ?? "Workspace";
  const selectedSubscription = subscriptions.find((item) => item.id === selectedSubscriptionId) ?? null;
  useEffect(() => { articlesRef.current = articles; }, [articles]);

  useEffect(() => {
    let active = true;
    if (signedIn !== null) return () => { active = false; };
    client.bootstrap().then((data) => { if (!active) return; articlesRef.current=data.articles; setSignedIn(true); setLoadError(""); setAccount(data.account); setWorkspaces(data.workspaces); setWorkspaceId(data.activeWorkspaceId); setSubscriptions(data.subscriptions); setArticles(data.articles); setSelectedId(data.articles[0]?.id ?? ""); setNewCount(data.newArticleCount); setArchived(data.workspaces.find((item) => item.id === data.activeWorkspaceId)?.archived ?? false); }).catch((error: Error) => { if (!active) return; if (error instanceof ApiError && error.status === 401) setSignedIn(false); else setLoadError(error.message); });
    return () => { active = false; if (noticeTimer.current) window.clearTimeout(noticeTimer.current); };
  }, [client, signedIn]);
  useEffect(() => { const unauthorized = () => setSignedIn(false); window.addEventListener("reader:unauthorized", unauthorized); return () => window.removeEventListener("reader:unauthorized", unauthorized); }, []);
  useEffect(()=>{const pop=()=>{const next=window.location.pathname;if(!confirmDiscard()){history.pushState({},"",locationPathRef.current);return}locationPathRef.current=next;setLocationPath(next)};window.addEventListener("popstate",pop);return()=>window.removeEventListener("popstate",pop)},[]);
  useEffect(() => {
    if (!accountMenuOpen) return;
    const closeOutside = (event: PointerEvent) => { if (!accountMenuRef.current?.contains(event.target as Node)) setAccountMenuOpen(false); };
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape") setAccountMenuOpen(false); };
    window.addEventListener("pointerdown", closeOutside);
    window.addEventListener("keydown", closeOnEscape);
    return () => { window.removeEventListener("pointerdown", closeOutside); window.removeEventListener("keydown", closeOnEscape); };
  }, [accountMenuOpen]);

  const filtered = useMemo(() => articles.filter((a) => {
    if (selectedSubscriptionId && !a.subscriptionIds?.includes(selectedSubscriptionId)) return false;
    if (view === "unread") return !a.read && !a.trash;
    if (view === "saved") return a.saved && !a.trash;
    if (view === "later") return a.later && !a.trash;
    if (view === "trash") return a.trash;
    return !a.trash;
  }), [articles, view, selectedSubscriptionId]);
  const selected = filtered.find((a) => a.id === selectedId) ?? filtered[0];
  filteredRef.current=filtered;selectedIdRef.current=selectedId;selectedRef.current=selected;

  const update = (id: string, patch: Partial<Pick<Article, "read" | "saved" | "later" | "trash">>) => {
    const before = articlesRef.current.find((item) => item.id === id); if (!before || !workspaceId) return;
    const mutationKeys=Object.keys(patch).map(key=>`${id}:${key}`);if(mutationKeys.some(key=>pendingArticleMutations.has(key)))return;
    setPendingArticleMutations(keys=>new Set([...keys,...mutationKeys]));
    const requestWorkspace = workspaceId;
    const generation = (mutationGeneration.current.get(id) ?? 0) + 1;
    mutationGeneration.current.set(id, generation);
    setArticles((items) => items.map((item) => item.id === id ? { ...item, ...patch } : item));
    const previous = mutationQueue.current.get(id) ?? Promise.resolve();
    const request = previous.catch(() => undefined).then(() => client.updateArticle(requestWorkspace, id, patch)).then((saved) => {
      if (requestWorkspace === workspaceId && mutationGeneration.current.get(id) === generation) setArticles((items) => items.map((item) => item.id === id ? saved : item));
    }).catch((error: Error) => {
      if (requestWorkspace === workspaceId && mutationGeneration.current.get(id) === generation) setArticles((items) => items.map((item) => item.id === id ? { ...item, ...Object.fromEntries(Object.keys(patch).map((key) => [key, before[key as keyof Article]])) } : item));
      announce(error.message);
    }).finally(() => { setPendingArticleMutations(keys=>{const next=new Set(keys);mutationKeys.forEach(key=>next.delete(key));return next});if (mutationQueue.current.get(id) === request) mutationQueue.current.delete(id); });
    mutationQueue.current.set(id, request);
  };
  const open = (id: string) => { setSelectedId(id); update(id, { read: true }); setMobilePanel("article"); };
  const chooseView = (next: View) => { setView(next); setSelectedSubscriptionId(null); setMobilePanel("list"); };
  const announce = (message: string) => { setNotice(message); if (noticeTimer.current) window.clearTimeout(noticeTimer.current); noticeTimer.current = window.setTimeout(() => setNotice(""), 1600); };
  const switchWorkspace = (id: string) => {
    if (id === workspaceId || switchingWorkspace) return;
    const generation = ++workspaceGeneration.current;
    setSwitchingWorkspace(true);
    Promise.all([client.listArticles(id, "all"), client.listSubscriptions(id)]).then(([nextArticles, nextSubscriptions]) => {
      if (generation !== workspaceGeneration.current) return;
      setWorkspaceId(id); setArticles(nextArticles); setSubscriptions(nextSubscriptions); setSelectedId(nextArticles[0]?.id ?? "");
      setSelectedSubscriptionId(null); setView("all"); setArchived(workspaces.find(item => item.id === id)?.archived ?? false); setNewCount(0); setMobilePanel("list");
    }).catch((error: Error) => { if (generation === workspaceGeneration.current) announce(error.message); }).finally(() => { if (generation === workspaceGeneration.current) setSwitchingWorkspace(false); });
  };
  const markAllRead = () => {
    if (markingAll || !workspaceId) return;
    const requestWorkspace = workspaceId;
    const affected = new Set(filtered.filter(article => !article.read).map(article => article.id));
    if (affected.size === 0) return;
    setMarkingAll(true);
    setArticles(items => items.map(article => affected.has(article.id) ? { ...article, read: true } : article));
    client.markAllRead(requestWorkspace, view, selectedSubscriptionId ?? undefined).catch((error: Error) => {
      if (requestWorkspace === workspaceId) setArticles(items => items.map(article => affected.has(article.id) ? { ...article, read: false } : article));
      announce(error.message);
    }).finally(() => { if (requestWorkspace === workspaceId) setMarkingAll(false); });
  };
  const navigate=(path:string)=>{if(path!==locationPathRef.current&&!confirmDiscard())return false;history.pushState({},"",path);locationPathRef.current=path;setLocationPath(path);return true};
  const subscriptionRoute=/^\/subscriptions(?:\/([^/]+))?$/.exec(locationPath);
  const openRef=useRef(open),updateRef=useRef(update);openRef.current=open;updateRef.current=update;
  useLayoutEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (event.target instanceof Element && event.target.matches("input,textarea,select")) return;
      const current=filteredRef.current,currentSelected=selectedRef.current;
      const at = current.findIndex((article) => article.id === selectedIdRef.current);
      if (event.key === "j" && current[at + 1]) openRef.current(current[at + 1].id);
      if (event.key === "k" && current[at - 1]) openRef.current(current[at - 1].id);
      if (event.key === "s" && currentSelected) updateRef.current(currentSelected.id, { saved: !currentSelected.saved });
      if (event.key === "l" && currentSelected) updateRef.current(currentSelected.id, { later: !currentSelected.later });
      if (event.key === "Escape") setMobilePanel("list");
    };
    window.addEventListener("keydown", keyboard); return () => window.removeEventListener("keydown", keyboard);
  }, []);

  if (loadError) return <main class="fatal-state"><h1>Reader is unavailable</h1><p>{loadError}</p><button class="primary-button" onClick={() => window.location.reload()}>Try again</button></main>;
  if (signedIn === null) return <main class="bootstrap-loading" role="status" aria-label="Opening your library" aria-busy="true"><span class="bootstrap-loading__spinner" aria-hidden="true"/></main>;
  if (!signedIn || /\/(invite|reset-password|change-password)/.test(window.location.pathname)) return <AuthScreen client={client} onSignedIn={() => setSignedIn(null)} />;
  return <div class={`app theme-${theme}`} data-theme={theme}>
    <header class="topbar">
      <button class="mobile-menu icon-button" aria-label="Open navigation" onClick={() => setMobilePanel("nav")}><Icon name="menu" /></button>
      <div class="brand"><span class="brand__mark"><Icon name="feed"/></span><span>Reader</span></div>
      <div class="topbar__spacer" />
      <button class="search-stub" disabled aria-describedby="search-description"><Icon name="search"/><span>Search</span><kbd>Coming later</kbd></button><span id="search-description" class="sr-only">Search is not available in this version.</span>
      <button class="icon-button" aria-label={`Use ${theme === "light" ? "dark" : "light"} theme`} onClick={() => setTheme(theme === "light" ? "dark" : "light")}><Icon name={theme === "light" ? "moon" : "sun"}/></button>
      <div class="account-menu" ref={accountMenuRef}>
        <button class="avatar" aria-label="Account menu" aria-haspopup="menu" aria-expanded={accountMenuOpen} onClick={() => setAccountMenuOpen((open) => !open)}>{account.initials}</button>
        {accountMenuOpen && <div class="account-popover" role="menu"><div class="account-popover__identity"><strong>{account.displayName}</strong><span>Administrator</span></div><button role="menuitem" aria-busy={signingOut} disabled={signingOut} onClick={() => { if (signingOut || !confirmDiscard()) return; setSigningOut(true); client.signOut().then(() => setSignedIn(false)).catch((error: Error) => announce(error.message)).finally(() => setSigningOut(false)); }}>{signingOut ? <><span class="spinner"/> Signing out…</> : "Sign out"}</button></div>}
      </div>
    </header>
    <main class={subscriptionRoute ? "entity-shell" : "reader-grid"}>
      {subscriptionRoute ? <SubscriptionsPage client={client} workspaceId={workspaceId} workspaceName={workspace} subscriptions={subscriptions} articles={articles} subscriptionId={subscriptionRoute[1] ? decodeURIComponent(subscriptionRoute[1]) : undefined} onBack={()=>navigate(subscriptionRoute[1]?"/subscriptions":"/")} onOpenDetail={(id)=>navigate(`/subscriptions/${encodeURIComponent(id)}`)} onOpenArticles={(id)=>{if(navigate("/")){setSelectedSubscriptionId(id);setView("all");setMobilePanel("list")}}} onRefresh={(id)=>client.refreshSubscription(id)} onPause={(id)=>{setSelectedSubscriptionId(id);setModal("pause")}} onChanged={(changed)=>setSubscriptions(items=>items.map(item=>item.id===changed.id?changed:item))} onEditRecipe={(recipe)=>{setEditingRecipe(recipe);setModal("webfeed")}} onDirtyNoteChange={(dirty)=>{dirtySubscriptionNote.current=dirty}}/> : <>
      <aside class={`sidebar panel-mobile-${mobilePanel === "nav" ? "show" : "hide"}`} aria-label="Reader navigation">
        <WorkspacePicker value={workspace} workspaces={workspaces} archived={archived} pending={switchingWorkspace} onChange={switchWorkspace} onArchive={() => setModal("archive")} />
        <nav class="nav-block" aria-label="Library">
          {views.map((item) => <button key={item.id} class={view === item.id && !selectedSubscriptionId ? "nav-item active" : "nav-item"} onClick={() => chooseView(item.id)}><Icon name={item.icon}/><span>{item.label}</span>{item.id === "unread" && <em>{articles.filter((a) => !a.read && !a.trash).length}</em>}</button>)}
          <button class="nav-item nav-item--disabled" disabled title="Search is coming later"><Icon name="search"/><span>Search</span><small>Later</small></button>
        </nav>
        <div class="sidebar__section-title"><button class="sidebar__section-link" onClick={()=>navigate("/subscriptions")}>Subscriptions</button><button class="icon-button icon-button--small" aria-label="Add subscription" onClick={() => setModal("add")}><Icon name="plus" size={16}/></button></div>
        <nav class="source-list" aria-label="Subscriptions">
          {subscriptions.filter(item=>item.status!=="archived").map((item) => <div class="source-entry" key={item.id}><button class={selectedSubscriptionId === item.id ? "source active" : "source"} title={item.error??item.continuation??(item.lastUpdate?`Last successful update: ${item.lastUpdate}`:"Waiting for the first successful update")} onClick={() => { setSelectedSubscriptionId(item.id); setView("all"); setMobilePanel("list"); }}><span class={`source__mark source__mark--${item.id}`}>{item.name.slice(0, 1)}</span><span>{item.name}{item.error?<small>Update failed · {item.error}</small>:item.incomplete?<small>Incomplete · continuation queued</small>:item.lastUpdate?<small>Updated {item.lastUpdate}</small>:<small>Not updated yet</small>}</span>{item.status === "paused" ? <Icon name="pause" size={14}/> : <em>{item.count}</em>}</button><button class="source-more" aria-label="More options" title={`More options for ${item.name}`} aria-expanded={sourceMenu===item.id} onClick={()=>setSourceMenu(sourceMenu===item.id?null:item.id)}><Icon name="dots" size={16}/></button>{sourceMenu===item.id&&<div class="source-menu"><button onClick={()=>{setSourceMenu(null);navigate(`/subscriptions/${encodeURIComponent(item.id)}`)}}>View details</button><button onClick={()=>{setSourceMenu(null);client.refreshSubscription(item.id).then(()=>announce("Refresh queued")).catch((e:Error)=>announce(e.message))}}>Refresh</button><button onClick={()=>{setSourceMenu(null);setSelectedSubscriptionId(item.id);setModal("pause")}}>Pause</button></div>}</div>)}
        </nav>
        <div class="sidebar__footer"><button class="nav-item" disabled={!selectedSubscription} title={selectedSubscription ? "Manage rules for this subscription" : "Choose a subscription first"} onClick={() => setModal("rules")}><Icon name="rule"/><span>Rules</span></button><button class="nav-item" onClick={() => setModal("shortcuts")}><Icon name="settings"/><span>Settings & shortcuts</span></button></div>
      </aside>
      <section class={`article-list panel-mobile-${mobilePanel === "list" ? "show" : "hide"}`} aria-label="Article list">
        <header class="list-header"><div><p class="eyebrow">{selectedSubscription ? "Subscription" : workspace}</p><h1>{selectedSubscription?.name ?? views.find((v) => v.id === view)?.label}</h1></div><div class="list-header__tools"><button class="icon-button" aria-label="Refresh subscription" aria-busy={refreshingSubscription} disabled={!selectedSubscription || refreshingSubscription} title={selectedSubscription ? "Refresh this subscription" : "Choose a subscription to refresh"} onClick={() => { if (!selectedSubscription || refreshingSubscription) return; setRefreshingSubscription(true); client.refreshSubscription(selectedSubscription.id).then(()=>announce("Refresh queued")).catch((error:Error)=>announce(error.message)).finally(()=>setRefreshingSubscription(false)); }}>{refreshingSubscription ? <span class="spinner"/> : <Icon name="refresh"/>}</button></div></header>
        {archived && <div class="archive-strip"><Icon name="archive"/><span>This workspace is archived. {workspaces.find(item=>item.id===workspaceId)?.archiveReason ? `Reason: ${workspaces.find(item=>item.id===workspaceId)?.archiveReason}. ` : ""}Your library remains readable.</span></div>}
        {selectedSubscription?.status==="paused"&&<div class="archive-strip"><Icon name="pause"/><span>Paused{selectedSubscription.reason?`: ${selectedSubscription.reason}`:""}{selectedSubscription.reasonAt?` · ${new Date(selectedSubscription.reasonAt).toLocaleString("en-US")}`:""}</span></div>}
        {newCount > 0 && <button class="new-items" disabled={showingNew} aria-busy={showingNew} onClick={() => {if(showingNew)return;const requestWorkspace=workspaceId;setShowingNew(true);client.listArticles(requestWorkspace,"all").then(next=>{if(requestWorkspace===workspaceId){setArticles(current=>{const byId=new Map(current.map(item=>[item.id,item]));next.forEach(item=>byId.set(item.id,item));return [...byId.values()]});setNewCount(0);announce("New articles added");}}).catch((error:Error)=>announce(error.message)).finally(()=>{if(requestWorkspace===workspaceId)setShowingNew(false)});}}><span>{showingNew?<><span class="spinner"/>Loading…</>:`${newCount} new articles`}</span><span>Show now</span></button>}
        <div class="list-controls"><button disabled={markingAll || !filtered.some(article => !article.read)} aria-busy={markingAll} onClick={markAllRead}>{markingAll ? <span class="spinner"/> : <Icon name="check" size={15}/>} {markingAll ? "Marking…" : "Mark all read"}</button><span>{filtered.length} articles</span><span>Newest first</span></div>
        <div class="article-scroll">
          {filtered.length ? filtered.map((article) => <ArticleRow article={article} selected={article.id === selected?.id} savePending={pendingArticleMutations.has(`${article.id}:saved`)} onOpen={() => open(article.id)} onSave={() => update(article.id, { saved: !article.saved })}/>) : <EmptyState view={view}/>} 
        </div>
      </section>
      {selected ? <ArticleReader article={selected} pending={pendingArticleMutations} className={`panel-mobile-${mobilePanel === "article" ? "show" : "hide"}`} onBack={() => setMobilePanel("list")} onUpdate={(patch) => update(selected.id, patch)} onRefresh={() => client.refreshFullText(workspaceId, selected.id)} onNotice={announce}/> : <section class="article-reader empty-reader"><Icon name="inbox" size={28}/><p>Select an article to read</p></section>}
      </>}
    </main>
    <div class="live-region" aria-live="polite">{notice}</div>
    {modal === "add" && <AddSubscription client={client} workspaceId={workspaceId} onClose={() => setModal(null)} onWebFeed={() => {setEditingRecipe(null);setModal("webfeed")}} onDone={(added) => { setSubscriptions(items => [...items, added]); setModal(null); announce("Subscription added"); }}/>} 
    {modal === "pause" && selectedSubscription && <ReasonDialog title="Pause subscription" action="Pause subscription" onClose={() => setModal(null)} onDone={(reason) => client.pauseSubscription(selectedSubscription.id, reason).then(() => { setSubscriptions(items => items.map(item => item.id === selectedSubscription.id ? { ...item, status: "paused", reason,reasonAt:new Date().toISOString() } : item)); setModal(null); announce("Subscription paused"); })}/>} 
    {modal === "archive" && !archived && <ReasonDialog title="Archive workspace" action="Archive workspace" onClose={() => setModal(null)} onDone={(reason) => client.archiveWorkspace(workspaceId, reason).then(() => { setArchived(true); setWorkspaces(items=>items.map(item=>item.id===workspaceId?{...item,archived:true,archiveReason:reason,archiveReasonAt:new Date().toISOString()}:item)); setModal(null); announce("Workspace archived"); })}/>} 
    {modal === "archive" && archived && <RestoreDialog onClose={() => setModal(null)} onDone={() => client.restoreWorkspace(workspaceId).then(() => { setArchived(false); setWorkspaces(items=>items.map(item=>item.id===workspaceId?{...item,archived:false}:item)); setModal(null); announce("Workspace restored"); })}/>} 
    {modal === "rules" && selectedSubscription && <RulesDialog client={client} workspaceId={workspaceId} subscriptionId={selectedSubscription.id} onClose={() => setModal(null)}/>} 
    {modal === "webfeed" && <WebFeedBuilder client={client} workspaceId={workspaceId} editing={editingRecipe} onClose={() => {setEditingRecipe(null);setModal(null)}} onDone={(added) => { setSubscriptions((items) => [...items, added]);setEditingRecipe(null); setModal(null); announce("Web feed created"); }} onUpdated={()=>{setEditingRecipe(null);setModal(null);announce("Web feed recipe updated; collection queued")}}/>} 
    {modal === "shortcuts" && <AdvancedSettings client={client} workspaceId={workspaceId} workspaceName={workspace} subscriptions={subscriptions} selectedSubscription={selectedSubscription} onSelectSubscription={setSelectedSubscriptionId} onSubscription={(changed)=>setSubscriptions(items=>items.map(item=>item.id===changed.id?changed:item))} onWorkspace={(item)=>{setWorkspaces(items=>[...items,item]);setWorkspaceId(item.id);setArticles([]);setSubscriptions([]);setSelectedSubscriptionId(null);}} onRename={(item)=>setWorkspaces(items=>items.map(old=>old.id===item.id?item:old))} onEditWebFeed={(recipe)=>{setEditingRecipe(recipe);setModal("webfeed")}} onClose={() => setModal(null)} onPause={() => setModal("pause")} onResume={() => {if(!selectedSubscription)return;client.resumeSubscription(selectedSubscription.id).then(()=>{setSubscriptions(items=>items.map(item=>item.id===selectedSubscription.id?{...item,status:"active",reason:undefined}:item));setModal(null);announce("Subscription resumed; catch-up queued");});}}/>} 
  </div>;
}

function WorkspacePicker({ value, workspaces, archived, pending, onChange, onArchive }: { value: string; workspaces: Workspace[]; archived: boolean; pending: boolean; onChange: (v: string) => void; onArchive: () => void }) {
  const [open, setOpen] = useState(false);
  return <div class="workspace-picker"><button class="workspace-button" disabled={pending} aria-busy={pending} onClick={() => setOpen(!open)} aria-expanded={open}>{pending ? <span class="spinner"/> : <span class="workspace-glyph">{value.slice(0,2).toUpperCase()}</span>}<span><small>Workspace</small><strong>{pending ? "Switching…" : value}</strong></span><span class="chevron">⌄</span></button>{open && <div class="workspace-menu">{workspaces.map((item) => <button key={item.id} disabled={pending} onClick={() => { onChange(item.id); setOpen(false); }}><span class="workspace-glyph">{item.name.slice(0,2)}</span>{item.name}{item.name === value && <Icon name="check" size={15}/>}</button>)}<hr/><button disabled={pending} onClick={onArchive}><Icon name={archived ? "refresh" : "archive"} size={16}/>{archived ? "Restore workspace" : "Archive workspace"}</button></div>}</div>;
}

function ArticleRow({ article, selected, savePending, onOpen, onSave }: { article: Article; selected: boolean; savePending:boolean;onOpen: () => void; onSave: () => void }) {
  return <article class={`article-row ${selected ? "selected" : ""} ${article.read ? "read" : ""}`}><button class="article-row__main" onClick={onOpen}><span class="unread-dot"/><span class="article-row__source">{article.sources?.join(" · ") ?? article.source}</span><time>{article.age}</time><h2>{article.title}</h2><p>{article.excerpt}</p><span class={`fulltext fulltext--${article.fullText}`}>{article.fullText === "ready" ? "Full text" : article.fullText === "pending" ? "Fetching full text" : "Excerpt only"}</span></button><button class={`row-save ${article.saved ? "active" : ""}`} disabled={savePending} aria-busy={savePending} aria-label={article.saved ? "Remove from saved" : "Save article"} onClick={onSave}>{savePending?<span class="spinner"/>:<Icon name="star" size={17}/>}</button></article>;
}

function ArticleReader({ article, pending, className, onBack, onUpdate, onRefresh, onNotice }: { article: Article; pending:Set<string>;className: string; onBack: () => void; onUpdate: (p: Partial<Article>) => void; onRefresh: () => Promise<void>; onNotice: (m: string) => void }) {
  const [refreshing,setRefreshing]=useState(false);
  return <article class={`article-reader ${className}`} aria-label="Article reader"><header class="reader-toolbar"><button class="icon-button reader-back" aria-label="Back to articles" onClick={onBack}><span aria-hidden="true">←</span></button><div class="reader-toolbar__states"><StateButton icon="unread" label={article.read ? "Mark unread" : "Mark read"} active={!article.read} pending={pending.has(`${article.id}:read`)} onClick={() => onUpdate({ read: !article.read })}/><StateButton icon="star" label={article.saved ? "Unsave" : "Save"} active={article.saved} pending={pending.has(`${article.id}:saved`)} onClick={() => onUpdate({ saved: !article.saved })}/><StateButton icon="later" label={article.later ? "Remove from later" : "Read later"} active={article.later} pending={pending.has(`${article.id}:later`)} onClick={() => onUpdate({ later: !article.later })}/><StateButton icon={article.trash ? "refresh" : "trash"} label={article.trash ? "Restore" : "Move to trash"} active={article.trash} pending={pending.has(`${article.id}:trash`)} onClick={() => onUpdate({ trash: !article.trash })}/></div><a class="icon-button" aria-label="Open original" href={article.url} target="_blank" rel="noopener noreferrer"><Icon name="external"/></a></header><div class="reader-body"><div class="reader-meta"><span class="source__mark">{article.source.slice(0,1)}</span><div><strong>{article.source}</strong><span>{article.age} ago · {article.author ?? "Unknown author"}</span></div></div><h1>{article.title}</h1><p class="reader-deck">{article.excerpt}</p>{article.fullText === "pending" && <div class="content-status"><span class="spinner"/> Fetching the full article. You can keep reading this excerpt.</div>}{article.fullText === "failed" && <div class="content-status content-status--error"><span>!</span> Full text was unavailable{article.fullTextReason ? ` (${article.fullTextReason})` : ""}. The source excerpt is preserved. <button disabled={refreshing} onClick={() => {if(refreshing)return;setRefreshing(true);onRefresh().then(()=>onNotice("Full-text refresh queued")).catch((error:Error)=>onNotice(error.message)).finally(()=>setRefreshing(false));}}>{refreshing?"Queuing…":"Try again"}</button></div>}{article.body.map((paragraph,index) => <p key={index}>{paragraph}</p>)}<footer class="article-sources"><span>Delivered by</span>{(article.sources ?? [article.source]).map((source) => <span class="source-chip" key={source}>{source}</span>)}</footer></div></article>;
}

function StateButton({ icon, label, active, pending, onClick }: { icon: IconName; label: string; active?: boolean; pending:boolean;onClick: () => void }) { return <button class={`icon-button ${active ? "active" : ""}`} disabled={pending} aria-busy={pending} aria-label={label} aria-pressed={active} onClick={onClick}>{pending?<span class="spinner"/>:<Icon name={icon}/>}</button>; }
function EmptyState({ view }: { view: View }) { return <div class="empty-state"><span><Icon name={view === "trash" ? "trash" : "check"} size={24}/></span><h2>Nothing here right now</h2><p>{view === "trash" ? "Articles you move to trash will stay recoverable here." : "You are caught up in this view."}</p></div>; }

function AddSubscription({ client, workspaceId, onClose, onWebFeed, onDone }: { client: ApiClient; workspaceId: string; onClose: () => void; onWebFeed: () => void; onDone: (item: Subscription) => void }) {
  const [url, setUrl] = useState(""); const [stage, setStage] = useState<"entry" | "preview">("entry"); const [pending, setPending] = useState(false); const [error,setError]=useState(""); const [preview,setPreview]=useState<{title:string;kind:string;articles:{title:string}[]}|null>(null);
  const discover = (event: Event) => { event.preventDefault(); if (!url || pending) return; const requestedUrl=url;setPending(true); setError(""); client.discoverFeed(url).then((value) => {if(url===requestedUrl){setPreview(value);setStage("preview");}}).catch((failure:Error)=>setError(failure.message)).finally(() => setPending(false)); };
  const add = () => { if(pending)return;setPending(true);setError("");client.addSubscription(workspaceId,url,preview?.title).then(onDone).catch((failure:Error)=>setError(failure.message)).finally(()=>setPending(false)); };
  return <Dialog title="Add a subscription" description="Paste a feed or public website URL. Nothing is added until you confirm." onClose={onClose}>
    {stage === "entry" ? <form onSubmit={discover}><div class="modal__body"><label>Feed or website URL<AutofillResistantField type="url" value={url} onInput={(e) => setUrl(e.currentTarget.value)} placeholder="https://example.com/feed.xml" required autoFocus/></label><span class="field-help field-help--error" role="alert">{error}</span><div class="scope-note"><Icon name="globe"/><span><strong>Safe discovery</strong><small>Redirects and every network hop are checked before connecting.</small></span></div></div><footer class="modal__actions"><button type="button" class="secondary-button" onClick={onWebFeed}>Build a Web feed</button><span/><button type="button" class="secondary-button" onClick={onClose}>Cancel</button><button class="primary-button" disabled={pending || !url}>{pending ? <><span class="spinner"/>Checking…</> : "Check URL"}</button></footer></form> : <><div class="modal__body"><div class="feed-preview"><span class="source__mark source__mark--rust">{preview?.title.slice(0,1)??"F"}</span><div><h3>{preview?.title??"Discovered feed"}</h3><p>{preview?.kind.replaceAll("_"," ")} · {url}</p></div><span class="success-badge"><Icon name="check" size={14}/>Feed found</span></div>{preview?.articles.length ? <div class="preview-articles"><small>AVAILABLE INITIAL ITEMS</small>{preview.articles.slice(0,5).map((item,index)=><div key={`${item.title}-${index}`}><strong>{item.title}</strong></div>)}</div>:null}<span class="field-help field-help--error" role="alert">{error}</span></div><footer class="modal__actions"><button class="secondary-button" disabled={pending} onClick={() => setStage("entry")}>Back</button><span/><button class="secondary-button" disabled={pending} onClick={onClose}>Cancel</button><button class="primary-button" disabled={pending} onClick={add}>{pending ? <><span class="spinner"/>Adding…</> : "Add subscription"}</button></footer></>}
  </Dialog>;
}

function ReasonDialog({ title, action, onClose, onDone }: { title: string; action: string; onClose: () => void; onDone: (reason: string) => Promise<void> }) {
  const [reason, setReason] = useState(""); const [pending, setPending] = useState(false); const [touched, setTouched] = useState(false); const [error, setError] = useState(""); const invalid = reason.trim().length === 0;
  const submit = (event: Event) => { event.preventDefault(); setTouched(true); if (invalid) return; setPending(true); setError(""); onDone(reason).catch((failure: Error) => setError(failure.message)).finally(() => setPending(false)); };
  const close = () => { if (!pending) onClose(); };
  return <Dialog title={title} description={title.startsWith("Archive") ? "Updates stop for every subscription. Your library and individual pauses stay intact." : "New delivery stops for this subscription. Existing articles remain available."} onClose={close}>
    <form onSubmit={submit} aria-busy={pending}><div class="modal__body"><label>Reason <span class="required">Required</span><AutofillResistantTextarea disabled={pending} value={reason} onInput={(e) => setReason(e.currentTarget.value)} onBlur={() => setTouched(true)} rows={4} placeholder="Why are you pausing updates?" autoFocus aria-invalid={touched && invalid} aria-describedby="reason-help"/><span id="reason-help" class={`field-help ${touched && invalid ? "field-help--error" : ""}`}>{touched && invalid ? "Enter a reason before continuing." : error || "Saved exactly as entered, with your name and the current time."}</span></label></div><footer class="modal__actions"><span/><span/><button type="button" class="secondary-button" disabled={pending} onClick={close}>Cancel</button><button class="danger-button" disabled={pending || (touched && invalid)}>{pending ? <><span class="spinner"/>Saving…</> : action}</button></footer></form>
  </Dialog>;
}

function RestoreDialog({ onClose, onDone }: { onClose: () => void; onDone: () => Promise<void> }) { const [pending,setPending]=useState(false),[error,setError]=useState("");const close=()=>{if(!pending)onClose()};return <Dialog title="Restore workspace" description="Subscriptions that were active before archiving will resume. Individually paused subscriptions remain paused." onClose={close}><div class="modal__body"><div class="scope-note"><Icon name="refresh"/><span><strong>Catch-up resumes from the saved boundary</strong><small>Articles no longer available from their source may not be recovered.</small></span></div><span class="field-help field-help--error" role="alert">{error}</span></div><footer class="modal__actions"><span/><span/><button class="secondary-button" disabled={pending} onClick={close}>Cancel</button><button class="primary-button" disabled={pending} onClick={() => {if(pending)return;setPending(true);setError("");onDone().catch((failure:Error)=>setError(failure.message)).finally(()=>setPending(false));}}>{pending ? <><span class="spinner"/>Restoring…</> : "Restore workspace"}</button></footer></Dialog>; }

function AdvancedSettings({client,workspaceId,workspaceName,subscriptions,selectedSubscription,onSelectSubscription,onSubscription,onWorkspace,onRename,onEditWebFeed,onClose,onPause,onResume}:{client:ApiClient;workspaceId:string;workspaceName:string;subscriptions:Subscription[];selectedSubscription:Subscription|null;onSelectSubscription:(id:string|null)=>void;onSubscription:(v:Subscription)=>void;onWorkspace:(w:Workspace)=>void;onRename:(w:Workspace)=>void;onEditWebFeed:(recipe:WebFeedRecipeView)=>void;onClose:()=>void;onPause:()=>void;onResume:()=>void}){
 const [opml,setOpml]=useState(""),[previewId,setPreviewId]=useState(""),[name,setName]=useState(workspaceName),[subscriptionName,setSubscriptionName]=useState(selectedSubscription?.name??""),[username,setUsername]=useState(""),[pending,setPending]=useState(false),[confirmUnsubscribe,setConfirmUnsubscribe]=useState(false),[message,setMessage]=useState("");
 const pendingRef=useRef(false);
 useEffect(()=>{setSubscriptionName(selectedSubscription?.name??"");setConfirmUnsubscribe(false)},[selectedSubscription?.id,selectedSubscription?.name]);
 const run=<T,>(work:()=>Promise<T>,done:(value:T)=>void)=>{if(pendingRef.current)return;pendingRef.current=true;setPending(true);setMessage("");work().then(done).catch((e:Error)=>setMessage(e.message)).finally(()=>window.setTimeout(()=>{pendingRef.current=false;setPending(false)},250));};
 const download=()=>run(()=>client.exportOpml(workspaceId),text=>{const url=URL.createObjectURL(new Blob([text],{type:"text/x-opml"}));const link=document.createElement("a");link.href=url;link.download=`${workspaceName}.opml`;link.click();URL.revokeObjectURL(url);setMessage("Export downloaded");});
 return <Dialog title="Workspace & account" description="Manage this workspace, portable subscriptions and administrator links." onClose={onClose} width="680px"><div class="modal__body settings-list"><section><h3>Workspace</h3><label>Name<AutofillResistantField value={name} onInput={e=>setName(e.currentTarget.value)}/></label><button class="settings-action" disabled={pending||!name.trim()} onClick={()=>run(()=>client.renameWorkspace(workspaceId,name),onRename)}>Rename current workspace</button><button class="settings-action" disabled={pending||!name.trim()} onClick={()=>run(()=>client.createWorkspace(name),onWorkspace)}><Icon name="plus"/>Create as new workspace</button></section><section><h3>Subscription</h3><label>Choose subscription<AutofillResistantSelect value={selectedSubscription?.id??""} onChange={e=>onSelectSubscription(e.currentTarget.value||null)}><option value="">Not selected</option>{subscriptions.map(item=><option value={item.id} key={item.id}>{item.name}{item.status==="archived"?" (archived)":""}</option>)}</AutofillResistantSelect></label>{selectedSubscription&&<><label>Custom name<AutofillResistantField value={subscriptionName} onInput={e=>setSubscriptionName(e.currentTarget.value)}/></label><button class="settings-action" disabled={pending||!subscriptionName||subscriptionName.trim()!==subscriptionName} onClick={()=>run(()=>client.renameSubscription(selectedSubscription.id,subscriptionName),onSubscription)}>Rename subscription</button>{selectedSubscription.editableWebFeed&&<button class="settings-action" disabled={pending} onClick={()=>run(()=>client.getWebFeedRecipe(selectedSubscription.id),onEditWebFeed)}>Edit Web feed recipe</button>}{selectedSubscription.status==="archived"?<button class="settings-action" disabled={pending} onClick={()=>run(()=>client.restoreSubscription(selectedSubscription.id),value=>{onSubscription(value);setMessage("Subscription restored; catch-up queued. Items no longer available at the source may not be recovered.")})}><Icon name="refresh"/>Restore subscription</button>:<><button class="settings-action" disabled={pending} onClick={selectedSubscription.status==="paused"?onResume:onPause}><Icon name={selectedSubscription.status==="paused"?"refresh":"pause"}/>{selectedSubscription.status==="paused"?`Resume ${selectedSubscription.name}`:`Pause ${selectedSubscription.name}`}</button><button class="settings-action" disabled={pending} onClick={()=>{if(!confirmUnsubscribe){setConfirmUnsubscribe(true);setMessage("Choose Unsubscribe again to confirm. Existing articles and full text stay in your library.");return}run(()=>client.unsubscribe(selectedSubscription.id),value=>{onSubscription(value);setMessage("Unsubscribed. Existing articles and full text remain available.")})}}><Icon name="archive"/>{confirmUnsubscribe?"Confirm unsubscribe":"Unsubscribe"}</button></>}</>}</section><section><h3>OPML</h3><label>OPML document<AutofillResistantTextarea rows={3} value={opml} onInput={e=>{setOpml(e.currentTarget.value);setPreviewId("")}}/></label><button class="settings-action" disabled={pending||!opml} onClick={()=>run(()=>client.importOpml(workspaceId,opml),result=>{setPreviewId(result.preview_id);setMessage(`${result.subscriptions} subscriptions ready${result.warnings.length ? `; ${result.warnings.join("; ")}` : ""}`);})}>Preview import</button><button class="settings-action" disabled={pending||!previewId} onClick={()=>run(()=>client.importOpml(workspaceId,opml,previewId),()=>{setPreviewId("");setMessage("Import applied");})}>Apply previewed import</button><button class="settings-action" disabled={pending} onClick={download}>Download OPML export</button></section><section><h3>Administrator links</h3><label>Username<AutofillResistantField value={username} onInput={e=>setUsername(e.currentTarget.value)}/></label><button class="settings-action" disabled={pending||!username} onClick={()=>run(()=>client.createInvite(username),result=>setMessage(`Invitation: ${result.url}`))}>Create invitation</button><button class="settings-action" disabled={pending||!username} onClick={()=>run(()=>client.createPasswordReset(username),result=>setMessage(`Password reset: ${result.url}`))}>Create password reset</button></section><span class="field-help" role="status">{pending?"Working…":message}</span></div><footer class="modal__actions"><span/><span/><span/><button class="primary-button" onClick={onClose}>Done</button></footer></Dialog>;
}
