import {useEffect,useRef,useState} from "preact/hooks";
import {useFloatingChat} from "../ai/useFloatingChat";
import {ChatCopyButton} from "../ai/ChatCopyButton";
import {Icon} from "../ui/Icon";
import type {EntityDefinition,KnownDefinition,DefinitionsView} from "../api/glossary";
import type {useGlossary} from "./useGlossary";
import {StyledParagraph} from "./StyledParagraph";
import "./glossary.css";

export function GlossaryPanel({controller}:{controller:ReturnType<typeof useGlossary>}){
  const floating=useFloatingChat(false);
  const [shown,setShown]=useState<DefinitionsView|null>(controller.view);
  const latest=useRef(controller.view),pressed=useRef(false),scrolling=useRef(false),timer=useRef<number>();
  const body=useRef<HTMLDivElement>(null),newBlock=useRef<HTMLDivElement>(null);
  latest.current=controller.view;
  const flush=()=>{const selection=window.getSelection();if(!pressed.current&&!scrolling.current&&!(selection&&!selection.isCollapsed&&body.current?.contains(selection.anchorNode)))setShown(latest.current);};
  useEffect(()=>{flush();},[controller.view]);
  useEffect(()=>{
    const release=()=>{pressed.current=false;flush();};
    window.addEventListener("pointerup",release);window.addEventListener("pointercancel",release);document.addEventListener("selectionchange",flush);
    floating.element.current?.querySelector<HTMLElement>(".glossary-close")?.focus();
    return()=>{clearTimeout(timer.current);window.removeEventListener("pointerup",release);window.removeEventListener("pointercancel",release);document.removeEventListener("selectionchange",flush);};
  },[]);
  const entities=shown?.job?.status==="completed"?shown.job.result.entities:[];
  const known=shown?.known??[],names=new Set(known.map(k=>k.definition.term)),fresh=entities.filter(e=>!names.has(e.name));
  const status=controller.busy?"Извлекаем термины…":controller.error||(shown?.job?.status==="failed"?shown.job.error:shown&&!shown.channel.indexReady?"Сначала импортируйте историю канала.":shown&&!shown.channel.generationAllowed?"DeepSeek недоступен для этого аккаунта.":"");
  return <div ref={floating.element} style={floating.style} class="glossary-panel" role="dialog" aria-modal="false" aria-label="Термины статьи" onPointerDown={()=>{pressed.current=true;}} onKeyDown={e=>{if(e.key==="Escape"){e.stopPropagation();controller.close();}}}>
    <header><div class="glossary-handle" role="button" tabIndex={0} aria-label="Переместить окно терминов" {...floating.handle}><small>Термины</small><strong>{controller.target?.title}</strong></div><button class="icon-button glossary-close" aria-label="Закрыть термины" onClick={controller.close}><Icon name="close"/></button></header>
    <div class="glossary-status" role="status" aria-busy={controller.busy}>{controller.busy&&<span class="spinner"/>}<span>{status}</span></div>
    <div ref={body} class="glossary-content" onScroll={()=>{scrolling.current=true;clearTimeout(timer.current);timer.current=window.setTimeout(()=>{scrolling.current=false;flush();},180);}}>
      <section><h3>Новые определения <span>{fresh.length}</span></h3><div ref={newBlock}>{fresh.map(e=><NewDefinition key={e.name} entity={e}/>)}</div>{shown?.job?.status==="completed"&&!fresh.length&&<p class="glossary-muted">Новых определений нет.</p>}</section>
      <hr/><section><h3>Уже объяснено в канале <span>{known.length}</span></h3>{known.map(k=><Known key={k.definition.term} value={k}/>)}{shown?.job?.status==="completed"&&!known.length&&<p class="glossary-muted">Совпадений по точному названию нет.</p>}</section>
    </div>
    <footer><span class="glossary-muted">{shown?.channel.posts??"—"} постов · {shown?.channel.configured?"бот подключён":"бот не подключён"}<small>Публичная история может быть неполной</small></span><button class="icon-button" aria-label="Обновить определения" title="Обновить определения · новый запрос DeepSeek" disabled={controller.busy||!shown?.channel.generationAllowed||!shown.channel.indexReady} onClick={()=>void controller.retry()}><Icon name="refresh"/></button><ChatCopyButton label="Скопировать все новые определения" text={fresh.map(e=>`**${e.name}** — ${e.explanation}`).join("\n\n")} html={()=>Array.from(newBlock.current?.querySelectorAll(".glossary-paragraph")??[]).map(p=>`<p>${p.innerHTML}</p>`).join("")}/></footer>
  </div>;
}
function NewDefinition({entity:e}:{entity:EntityDefinition}){
  const paragraph=useRef<HTMLParagraphElement>(null);
  return <article class="glossary-entry"><p class="glossary-paragraph" ref={paragraph}><strong>{e.name}</strong> — {e.explanation}</p>{e.insufficientContext&&<small class="glossary-muted">Недостаточно контекста для уверенного определения</small>}<ChatCopyButton label={`Скопировать ${e.name}`} text={`**${e.name}** — ${e.explanation}`} html={()=>`<p>${paragraph.current?.innerHTML??""}</p>`}/></article>;
}
function Known({value:k}:{value:KnownDefinition}){
  const paragraph=useRef<HTMLDivElement>(null);
  return <article class="glossary-entry"><div ref={paragraph}><StyledParagraph value={k.definition.paragraph}/></div><a href={k.permalink} target="_blank" rel="noopener noreferrer">Пост в канале ↗</a>{k.stale&&<small class="glossary-muted">Есть конфликт обновления — показана сохранённая версия</small>}<ChatCopyButton label={`Скопировать известное определение ${k.definition.term}`} text={k.definition.paragraph.text} html={()=>paragraph.current?.innerHTML??""}/></article>;
}
