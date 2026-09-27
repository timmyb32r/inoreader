import type {ComponentChildren} from "preact";
import type {StyledText} from "../api/glossary";

/** Telegram entities use UTF-16 offsets, exactly JavaScript's string indexing.
 * Render text nodes and a closed element set; never inject archived HTML. */
export function StyledParagraph({value}:{value:StyledText}){
  const cuts=[...new Set([0,value.text.length,...value.marks.flatMap(m=>[m.start,m.end])])].sort((a,b)=>a-b);
  return <span class="glossary-paragraph">{cuts.slice(0,-1).map((start,i)=>{
    const end=cuts[i+1];let node:ComponentChildren=value.text.slice(start,end);
    for(const {style} of value.marks.filter(m=>m.start<=start&&m.end>=end)){
      switch(style.kind){
        case "bold":node=<strong>{node}</strong>;break;
        case "italic":node=<em>{node}</em>;break;
        case "underline":node=<u>{node}</u>;break;
        case "strike":node=<s>{node}</s>;break;
        case "code":case "pre":node=<code>{node}</code>;break;
        case "quote":node=<span class="glossary-quote">{node}</span>;break;
        case "spoiler":node=<span class="glossary-spoiler">{node}</span>;break;
        case "link":if(/^https?:\/\//i.test(style.url))node=<a href={style.url} target="_blank" rel="noopener noreferrer">{node}</a>;break;
      }
    }return <span key={start}>{node}</span>;
  })}</span>;
}
