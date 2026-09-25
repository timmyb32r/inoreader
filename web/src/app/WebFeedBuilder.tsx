import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { ApiClient, FeedPreview, SelectorDraft, VisualPreview, WebFeedDraft,WebFeedRecipeView } from "../api/client";
import type { Subscription } from "./data";
import { Dialog } from "./Dialog";
import { Icon } from "../ui/Icon";
import { AutofillResistantField, AutofillResistantSelect } from "../ui/fields";

type Language = SelectorDraft["language"];
type Loading = WebFeedDraft["loading"];
type Viewport = WebFeedDraft["viewport"];
type SelectorRow = SelectorDraft & { key: number };

const emptySelector = (key: number): SelectorRow => ({ key, language: "css", expression: "" });

export function WebFeedBuilder({ client, workspaceId, editing,onClose, onDone,onUpdated }: { client: ApiClient; workspaceId: string;editing?:WebFeedRecipeView|null; onClose: () => void; onDone: (item: Subscription) => void;onUpdated?:()=>void }) {
  const initial=editing?.draft;
  const [url, setUrl] = useState(initial?.url??"");
  const [selector, setSelector] = useState(initial?.selector??"");
  const [selectorLanguage, setSelectorLanguage] = useState<Language>(initial?.selectorLanguage??"css");
  const [loading, setLoading] = useState<Loading>(initial?.loading??"automatic");
  const [viewport, setViewport] = useState<Viewport>(initial?.viewport??"desktop");
  const [listingUrl,setListingUrl]=useState(initial?.listingUrl??"");
  const [cardSelector,setCardSelector]=useState(initial?.cardSelector??"");
  const [titleSelector,setTitleSelector]=useState(initial?.titleSelector??"");
  const [dateSelector,setDateSelector]=useState(initial?.dateSelector??"");
  const [contentSelector,setContentSelector]=useState(initial?.contentSelector??"");
  const [waitSelector,setWaitSelector]=useState(initial?.waitSelector??"");
  const [urlPattern,setUrlPattern]=useState(initial?.urlPattern??"");
  const [maxPages,setMaxPages]=useState(initial?.maxPages??1);
  const [overlays, setOverlays] = useState<SelectorRow[]>(()=>(initial?.hideOverlays??[]).map((v,i)=>({...v,key:i+3})));
  const [startPages, setStartPages] = useState<{ key: number; url: string }[]>(()=>(initial?.startPages??[]).map((url,i)=>({url,key:i+100})));
  const [nextPage, setNextPage] = useState<SelectorRow>(() => initial?.nextPage?{...initial.nextPage,key:1}:emptySelector(1));
  const [loadMore, setLoadMore] = useState<SelectorRow>(() => initial?.loadMore?{...initial.loadMore,key:2}:emptySelector(2));
  const [loadMoreClicks, setLoadMoreClicks] = useState(initial?.loadMoreClicks??0);
  const [scrolls, setScrolls] = useState(initial?.scrolls??0);
  const [preview, setPreview] = useState<FeedPreview | null>(null);
  const [previewFingerprint, setPreviewFingerprint] = useState("");
  const [visual, setVisual] = useState<VisualPreview | null>(null);
  const [visualFingerprint, setVisualFingerprint] = useState("");
  const [selectedCount, setSelectedCount] = useState<number | null>(null);
  const [pending, setPending] = useState<"preview" | "create" | "visual" | "select" | null>(null);
  const [error, setError] = useState("");
  const [nextKey, setNextKey] = useState(3);
  const currentFingerprint = useRef("");
  const currentVisualKey = useRef("");
  const mounted = useRef(true);

  const draft = useMemo<WebFeedDraft>(() => ({
    workspaceId, url, selector, loading, selectorLanguage, viewport,
    listingUrl:listingUrl||undefined,cardSelector:cardSelector||undefined,titleSelector:titleSelector||undefined,dateSelector:dateSelector||undefined,contentSelector:contentSelector||undefined,waitSelector:waitSelector||undefined,urlPattern:urlPattern||undefined,maxPages,
    hideOverlays: overlays.filter(row => row.expression.length > 0).map(({ language, expression }) => ({ language, expression })),
    startPages: startPages.filter(page => page.url.length > 0).map(page => page.url),
    nextPage: nextPage.expression.length > 0 ? { language: nextPage.language, expression: nextPage.expression } : undefined,
    loadMore: loadMore.expression.length > 0 ? { language: loadMore.language, expression: loadMore.expression } : undefined,
    loadMoreClicks: loadMore.expression.length > 0 ? loadMoreClicks : 0,
    scrolls,
  }), [workspaceId, url, selector, loading, selectorLanguage, viewport,listingUrl,cardSelector,titleSelector,dateSelector,contentSelector,waitSelector,urlPattern,maxPages, overlays, startPages, nextPage, loadMore, loadMoreClicks, scrolls]);
  const fingerprint = JSON.stringify(draft);
  const browserActions = overlays.some(row=>row.expression.length>0) || startPages.some(page=>page.url.length>0) || nextPage.expression.length > 0 || loadMore.expression.length > 0 || scrolls > 0;
  const incompatibleStatic = loading === "static" && (selectorLanguage === "xpath" || browserActions);
  const valid = url.length > 0 && selector.length > 0 && Number.isSafeInteger(maxPages)&&maxPages>0&&Number.isSafeInteger(loadMoreClicks) && loadMoreClicks >= 0 && Number.isSafeInteger(scrolls) && scrolls >= 0 && !incompatibleStatic;
  const previewCurrent = preview !== null && previewFingerprint === fingerprint;
  const visualKey = JSON.stringify({ url, viewport });
  const visualCurrent = visual !== null && visualFingerprint === visualKey;
  currentFingerprint.current = fingerprint;
  currentVisualKey.current = visualKey;
  useEffect(() => () => { mounted.current = false; }, []);

  const mutate = (work: () => void) => { work(); setError(""); };
  const addOverlay = () => mutate(() => { setOverlays(rows => [...rows, emptySelector(nextKey)]); setNextKey(key => key + 1); });
  const addStartPage = () => mutate(() => { setStartPages(rows => [...rows, { key: nextKey, url: "" }]); setNextKey(key => key + 1); });
  const run = (create: boolean) => {
    if (pending || !valid || (create && !previewCurrent)) return;
    const requestedFingerprint = fingerprint;
    setPending(create ? "create" : "preview"); setError("");
    (create ? (editing?client.updateWebFeedRecipe(editing.subscriptionId,editing.version,draft):client.createWebFeed(draft)) : client.previewWebFeed(draft))
      .then(result => { if (!mounted.current || currentFingerprint.current !== requestedFingerprint) return; if (create){if(editing)onUpdated?.();else onDone(result as Subscription)} else { setPreview(result as FeedPreview); setPreviewFingerprint(requestedFingerprint); } })
      .catch((failure: Error) => { if (mounted.current && currentFingerprint.current === requestedFingerprint) setError(failure.message); })
      .finally(() => { if (mounted.current) setPending(null); });
  };
  const captureVisual = () => { if (pending || !url) return;const requestedVisualKey=visualKey;setPending("visual");setError("");client.visualPreview(workspaceId,url,viewport).then(value => {if(!mounted.current||currentVisualKey.current!==requestedVisualKey)return;setVisual(value);setVisualFingerprint(requestedVisualKey);setSelectedCount(null);}).catch((failure:Error)=>{if(mounted.current&&currentVisualKey.current===requestedVisualKey)setError(failure.message);}).finally(()=>{if(mounted.current)setPending(null);}); };
  const choose = (x:number,y:number) => {if(pending||!visualCurrent)return;const requestedToken=visual.snapshotToken;const requestedVisualKey=visualKey;setPending("select");setError("");client.visualSelect(workspaceId,requestedToken,x,y).then(value=>{if(!mounted.current||currentVisualKey.current!==requestedVisualKey)return;setSelectorLanguage(value.selector.language);setSelector(value.selector.expression);setSelectedCount(value.count);setVisual(null);setVisualFingerprint("");}).catch((failure:Error)=>{if(mounted.current&&currentVisualKey.current===requestedVisualKey)setError(failure.message);}).finally(()=>{if(mounted.current)setPending(null);});};
  const chooseFromImage = (event:MouseEvent) => {if(!visualCurrent)return;const image=event.currentTarget as HTMLImageElement;const bounds=image.getBoundingClientRect();const scale=Math.min(bounds.width/visual.width,bounds.height/visual.height);const renderedWidth=visual.width*scale;const renderedHeight=visual.height*scale;const x=(event.clientX-bounds.left-(bounds.width-renderedWidth)/2)/scale;const y=(event.clientY-bounds.top-(bounds.height-renderedHeight)/2)/scale;if(x>=0&&y>=0&&x<=visual.width&&y<=visual.height)choose(x,y);};

  return <Dialog title={editing?"Edit Web feed":"Build a Web feed"} description="Define the page, extraction selector and bounded browser actions. Preview the exact recipe on the server before saving it." onClose={onClose} width="980px">
    <div class="builder-body builder-body--advanced" aria-busy={pending !== null}>
      <section class="builder-controls">
        <fieldset><legend>Source</legend>
          <label>Public page URL<AutofillResistantField type="url" value={url} onInput={event => mutate(() => setUrl(event.currentTarget.value))}/></label>
          <div class="builder-pair"><label>Loading mode<AutofillResistantSelect value={loading} onChange={event => mutate(() => setLoading(event.currentTarget.value as Loading))}><option value="automatic">Automatic</option><option value="static">Static HTML</option><option value="browser">JavaScript browser</option></AutofillResistantSelect></label><label>Viewport<AutofillResistantSelect value={viewport} onChange={event => mutate(() => setViewport(event.currentTarget.value as Viewport))}><option value="desktop">Desktop</option><option value="mobile">Mobile</option></AutofillResistantSelect></label></div>
        </fieldset>
        <fieldset><legend>Articles</legend><SelectorEditor label="Link selector" value={{ key: 0, language: selectorLanguage, expression: selector }} onChange={value => mutate(() => { setSelectorLanguage(value.language); setSelector(value.expression); setSelectedCount(null); })}/><button type="button" class="secondary-button visual-capture" disabled={pending!==null||!url} onClick={captureVisual}>{pending==="visual"?<><span class="spinner"/>Capturing…</>:"Select from page image"}</button>{selectedCount!==null&&<p class="field-help">Validated repeated group: {selectedCount} similar items.</p>}</fieldset>
        <details><summary>Extraction fields <span>Optional</span></summary><div class="builder-detail">
          <label>Listing URL<AutofillResistantField type="url" value={listingUrl} onInput={e=>mutate(()=>setListingUrl(e.currentTarget.value))}/></label>
          <label>Card/container selector<AutofillResistantField value={cardSelector} onInput={e=>mutate(()=>setCardSelector(e.currentTarget.value))}/></label>
          <label>Title selector<AutofillResistantField value={titleSelector} onInput={e=>mutate(()=>setTitleSelector(e.currentTarget.value))}/></label>
          <label>Date selector<AutofillResistantField value={dateSelector} onInput={e=>mutate(()=>setDateSelector(e.currentTarget.value))}/></label>
          <label>Article text selector<AutofillResistantField value={contentSelector} onInput={e=>mutate(()=>setContentSelector(e.currentTarget.value))}/></label>
          <label>Wait selector<AutofillResistantField value={waitSelector} onInput={e=>mutate(()=>setWaitSelector(e.currentTarget.value))}/></label>
          <label>Article URL pattern<AutofillResistantField value={urlPattern} onInput={e=>mutate(()=>setUrlPattern(e.currentTarget.value))}/></label>
          <label>Maximum listing pages<AutofillResistantField type="number" min={1} step={1} value={maxPages} onInput={e=>mutate(()=>setMaxPages(e.currentTarget.valueAsNumber))}/></label>
        </div></details>
        <details><summary>Browser actions <span>{browserActions ? "Configured" : "Optional"}</span></summary>
          <div class="builder-detail">
            <h3>Hide overlays</h3>{overlays.map((row, index) => <div class="builder-repeat" key={row.key}><SelectorEditor label={`Overlay ${index + 1}`} value={row} onChange={value => mutate(() => setOverlays(rows => rows.map(item => item.key === row.key ? value : item)))}/><button type="button" class="icon-button icon-button--small" aria-label={`Remove overlay ${index + 1}`} onClick={() => mutate(() => setOverlays(rows => rows.filter(item => item.key !== row.key)))}><Icon name="trash" size={14}/></button></div>)}<button type="button" class="builder-add" onClick={addOverlay}><Icon name="plus" size={14}/>Add overlay selector</button>
            <h3>Additional start pages</h3>{startPages.map((page, index) => <div class="builder-repeat" key={page.key}><label>Start page {index + 1}<AutofillResistantField type="url" value={page.url} onInput={event => mutate(() => setStartPages(rows => rows.map(item => item.key === page.key ? { ...item, url: event.currentTarget.value } : item)))}/></label><button type="button" class="icon-button icon-button--small" aria-label={`Remove start page ${index + 1}`} onClick={() => mutate(() => setStartPages(rows => rows.filter(item => item.key !== page.key)))}><Icon name="trash" size={14}/></button></div>)}<button type="button" class="builder-add" onClick={addStartPage}><Icon name="plus" size={14}/>Add start page</button>
            <SelectorEditor label="Next page selector" optional value={nextPage} onChange={value => mutate(() => setNextPage(value))}/>
            <SelectorEditor label="Load more selector" optional value={loadMore} onChange={value => mutate(() => setLoadMore(value))}/>
            <div class="builder-pair"><label>Load more clicks<AutofillResistantField type="number" min={0} step={1} value={loadMoreClicks} disabled={!loadMore.expression} onInput={event => mutate(() => setLoadMoreClicks(event.currentTarget.valueAsNumber))}/></label><label>Full-page scrolls<AutofillResistantField type="number" min={0} step={1} value={scrolls} onInput={event => mutate(() => setScrolls(event.currentTarget.valueAsNumber))}/></label></div>
            <p class="field-help">The server applies its configured page and action limits before fetching.</p>
          </div>
        </details>
        <span class={`field-help ${error || incompatibleStatic ? "field-help--error" : ""}`} role="alert">{error || (incompatibleStatic ? "Static HTML cannot run XPath or browser actions. Choose Automatic or JavaScript browser." : "Changes after preview must be previewed again.")}</span>
      </section>
      <section class="site-preview" aria-label="Server preview">{visualCurrent?<div class="visual-selector"><small>SERVER IMAGE · CLICK A REPEATED ITEM</small><div class="visual-image-frame" style={{aspectRatio:`${visual.width}/${visual.height}`}}><img src={visual.imageDataUrl} width={visual.width} height={visual.height} alt="Server-rendered page for visual article selection" onClick={chooseFromImage}/></div><div class="visual-groups" aria-label="Detected repeated groups">{visual.groups.map(group=>{const box=group.boxes[0];return <button type="button" class="secondary-button" disabled={pending!==null||!box} onClick={()=>box&&choose(box.x+box.width/2,box.y+box.height/2)} key={group.id}>{group.count} × {group.selector.expression}</button>})}</div></div>:previewCurrent ? <div class="preview-articles"><small>SERVER PREVIEW · {preview.kind}</small><h3>{preview.title}</h3>{preview.articles.map((item, index) => <div key={`${item.title}-${index}`}><strong>{item.title}</strong><span>{item.publishedAt ?? "Date unavailable"}</span></div>)}</div> : <div class="empty-state"><Icon name="globe"/><h2>{visual ? "Page image changed" : preview ? "Recipe changed" : "No preview yet"}</h2><p>{visual ? "Capture the current URL and viewport again." : preview ? "Preview the updated recipe before creating the feed." : "Configure the recipe, then validate it against the live page."}</p></div>}</section>
    </div>
    <footer class="modal__actions"><button type="button" class="secondary-button" disabled={pending !== null || !valid} onClick={() => run(false)}>{pending === "preview" ? <><span class="spinner"/>Previewing…</> : "Preview on server"}</button><span/><button type="button" class="secondary-button" disabled={pending !== null} onClick={onClose}>Cancel</button><button type="button" class="primary-button" disabled={pending !== null || !previewCurrent} onClick={() => run(true)}>{pending === "create" ? <><span class="spinner"/>Saving…</> : editing?"Save recipe":"Create Web feed"}</button></footer>
  </Dialog>;
}

function SelectorEditor({ label, optional, value, onChange }: { label: string; optional?: boolean; value: SelectorRow; onChange: (value: SelectorRow) => void }) {
  return <div class="selector-editor"><div class="selector-editor__label">{label}{optional && <span class="optional">Optional</span>}</div><div><AutofillResistantSelect aria-label={`${label} language`} value={value.language} onChange={event => onChange({ ...value, language: event.currentTarget.value as Language })}><option value="css">CSS</option><option value="xpath">XPath</option></AutofillResistantSelect><AutofillResistantField aria-label={label} value={value.expression} onInput={event => onChange({ ...value, expression: event.currentTarget.value })}/></div></div>;
}
