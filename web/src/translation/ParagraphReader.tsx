import type { ComponentChildren } from "preact";
import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import type { AiClient, ParagraphJob, ParagraphTranslation, TranslationSegment } from "../api/ai";
import { Icon } from "../ui/Icon";
import { annotate, clearAnnotations, paragraphs } from "./annotations";
import "./translation.css";

type Word=Extract<TranslationSegment,{kind:"word"}>;
type Props={client:AiClient;workspaceId:string;articleId:string;html:string;body:string[];enabled:boolean;title?:string;excerpt?:string;children?:ComponentChildren};
export function ParagraphReader({client,workspaceId,articleId,html,body,enabled,title,excerpt,children}:Props) {
  const root=useRef<HTMLDivElement>(null),glossary=useRef(new Map<string,Word>());
  const busy=useRef(false),mounted=useRef(true),pressed=useRef(false),lastScroll=useRef(0);
  const [jobs,setJobs]=useState<ParagraphJob[]>([]),[selection,setSelection]=useState("");
  const [loading,setLoading]=useState(false),[error,setError]=useState("");
  const [word,setWord]=useState<{value:Word;left:number;top:number;anchor:DOMRect}|null>(null);
  const tooltip=useRef<HTMLDivElement>(null);
  const completedSignature=JSON.stringify(jobs.filter(j=>j.status==="completed"));
  const active=jobs.some(j=>j.status==="queued"||j.status==="generating");
  const current=jobs.find(j=>j.source===selection);
  const operation=useRef<{source:string;id:string}|null>(null);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{
    if(!enabled)return;
    let cancelled=false,timer:ReturnType<typeof setTimeout>;
    const load=async()=>{try{const values=await client.translations(workspaceId,articleId);if(!cancelled){setJobs(values);setError("");if(values.some(j=>j.status==="queued"||j.status==="generating"))timer=setTimeout(load,700);}}catch(e){if(!cancelled){setError((e as Error).message);timer=setTimeout(load,1500);}}};
    void load();return()=>{cancelled=true;clearTimeout(timer);};
  },[client,workspaceId,articleId,enabled,active]);
  useEffect(()=>{
    const down=()=>{pressed.current=true;},up=()=>{pressed.current=false;},scroll=()=>{lastScroll.current=Date.now();setWord(null);};
    document.addEventListener("pointerdown",down);document.addEventListener("pointerup",up);document.addEventListener("pointercancel",up);window.addEventListener("scroll",scroll,true);window.addEventListener("resize",scroll);
    return()=>{document.removeEventListener("pointerdown",down);document.removeEventListener("pointerup",up);document.removeEventListener("pointercancel",up);window.removeEventListener("scroll",scroll,true);window.removeEventListener("resize",scroll);};
  },[]);
  useEffect(()=>{
    let timer:ReturnType<typeof setTimeout>,cancelled=false;
    const update=()=>{
      if(cancelled||!root.current)return;
      // Defer DOM annotation while the user selects text, presses, or scrolls.
      // Wrappers have exactly the source font/spacing and never add layout boxes.
      if(pressed.current||window.getSelection()?.isCollapsed===false||Date.now()-lastScroll.current<180){timer=setTimeout(update,180);return;}
      setWord(null);
      if(!enabled){clearAnnotations(root.current);return;}
      const results=new Map<string,ParagraphTranslation>();
      for(const job of jobs)if(job.status==="completed"&&!results.has(job.source))results.set(job.source,job.result);
      try{glossary.current=annotate(root.current,results);}catch(e){setError((e as Error).message);}
      paragraphs(root.current).forEach(p=>p.classList.toggle("paragraph-selected",p.textContent===selection));
    };
    update();return()=>{cancelled=true;clearTimeout(timer);};
  },[enabled,completedSignature,html,title,excerpt,selection]);
  useEffect(()=>{if(!enabled){setWord(null);setSelection("");}},[enabled]);
  useEffect(()=>{
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"){setWord(null);setSelection("");}};
    document.addEventListener("keydown",escape);return()=>document.removeEventListener("keydown",escape);
  },[]);
  async function translate(source:string) {
    if(busy.current)return;
    setSelection(source);setWord(null);setError("");
    const existing=jobs.find(j=>j.source===source);
    if(existing&&existing.status!=="failed")return;
    busy.current=true;setLoading(true);
    if(operation.current?.source!==source)operation.current={source,id:crypto.randomUUID()};
    try{
      const job=await client.translate(workspaceId,articleId,operation.current.id,source);
      if(mounted.current){setJobs(values=>[job,...values.filter(j=>j.id!==job.id)]);operation.current=null;}
    }catch(e){if(mounted.current)setError((e as Error).message);}
    finally{busy.current=false;if(mounted.current)setLoading(false);}
  }
  function hovered(target:EventTarget|null) {
    if(!enabled)return;
    const span=target instanceof Element?target.closest<HTMLElement>("[data-translation-word]"):null;
    const value=span?glossary.current.get(span.dataset.translationWord!):null;
    if(!value||!span){setWord(null);return;}
    const r=span.getBoundingClientRect();
    setWord({value,left:Math.max(12,Math.min(window.innerWidth-272,r.left+r.width/2-130)),top:Math.max(12,r.top-150),anchor:r});
  }
  useLayoutEffect(()=>{
    if(!word||!tooltip.current)return;
    const box=tooltip.current.getBoundingClientRect(),anchor=word.anchor;
    const readerLeft=root.current?.closest(".article-reader")?.getBoundingClientRect().left??0;
    const left=Math.max(12,Math.min(window.innerWidth-box.width-12,Math.max(readerLeft+12,anchor.left+anchor.width/2-box.width/2)));
    const preferred=anchor.top-box.height-10;
    const top=Math.max(12,Math.min(window.innerHeight-box.height-12,preferred>=12?preferred:anchor.bottom+10));
    if(left!==word.left||top!==word.top)setWord({...word,left,top});
  },[word]);
  const pending=loading||current?.status==="queued"||current?.status==="generating";
  return <>
    <div ref={root} class={`paragraph-reader ${enabled?"paragraph-translation-mode":""}`} onMouseOver={event=>hovered(event.target)} onMouseLeave={()=>setWord(null)} onFocusIn={event=>hovered(event.target)} onFocusOut={()=>setWord(null)} onClick={event=>{
      if(!enabled)return;
      const target=event.target instanceof Element?event.target:null;
      if(target?.closest("[data-translation-word]")){event.preventDefault();hovered(target);return;}
      const paragraph=target?.closest<HTMLElement>("[data-translatable]");
      if(paragraph&&window.getSelection()?.isCollapsed!==false){event.preventDefault();void translate(paragraph.textContent??"");}
    }} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){const span=(event.target as Element).closest("[data-translation-word]");if(span){event.preventDefault();hovered(span);}}}}>{title&&<h1>{title}</h1>}{excerpt&&<p class="reader-deck">{excerpt}</p>}{children}<div class="article-content" dangerouslySetInnerHTML={html?{__html:html}:undefined}>{!html&&body.map((p,i)=><p key={i}>{p}</p>)}</div></div>
    {enabled&&selection&&<section class="paragraph-result" role="region" aria-label="Paragraph translation" aria-busy={pending}>
      <header><strong>Перевод абзаца</strong><button class="icon-button" aria-label="Close paragraph translation" onClick={()=>setSelection("")}><Icon name="close"/></button></header>
      <div class="paragraph-result-body" aria-live="polite">{pending?<div class="paragraph-wait"><span class="spinner"/>Переводим абзац…</div>:error||current?.status==="failed"?<div role="alert">{error||(current?.status==="failed"?current.error:"")}</div>:current?.status==="completed"?<p>{current.result.translation}</p>:null}</div>
      <footer><span>DeepSeek · русский</span><button class="text-button" disabled={pending||(!error&&current?.status!=="failed")} onClick={()=>{if(current?.status==="failed")operation.current=null;void translate(selection);}}>Повторить</button></footer>
    </section>}
    {enabled&&error&&!selection&&<div class="paragraph-error" role="alert">{error}</div>}
    {enabled&&word&&<div ref={tooltip} class="word-card" role="tooltip" id="paragraph-word-card" style={{left:word.left,top:word.top}}><strong lang="zh">{word.value.source}</strong>{word.value.pinyin&&<span class="word-pinyin">{word.value.pinyin}</span>}<span class="word-meaning">{word.value.translation}</span></div>}
  </>;
}
