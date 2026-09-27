import type { ParagraphTranslation, TranslationSegment } from "../api/ai";

export const paragraphSelector = "p, li, h1, h2, h3, h4, h5, h6";
export function paragraphs(root:HTMLElement):HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(paragraphSelector)].filter(p => !p.querySelector(`${paragraphSelector}, pre`) && !!p.textContent?.trim());
}
export function clearAnnotations(root:HTMLElement) {
  root.querySelectorAll("[data-translation-word]").forEach(span => span.replaceWith(...span.childNodes));
  root.querySelectorAll("[data-translatable]").forEach(p => {p.removeAttribute("data-translatable");p.classList.remove("paragraph-selected");});
  root.normalize();
}
/** Wrap existing text nodes only. No HTML replacement, trimming, inserted spaces,
 * or changes to links/emphasis. A word crossing inline elements shares one ID. */
export function annotate(root:HTMLElement, results:Map<string,ParagraphTranslation>):Map<string,Extract<TranslationSegment,{kind:"word"}>> {
  clearAnnotations(root);
  const glossary=new Map<string,Extract<TranslationSegment,{kind:"word"}>>();
  let ordinal=0;
  for(const paragraph of paragraphs(root)) {
    paragraph.dataset.translatable="true";
    const source=paragraph.textContent ?? "",result=results.get(source);
    if(!result)continue;
    if(result.source!==source || result.segments.map(s=>s.source).join("")!==source)throw new Error("Translation does not match the original paragraph.");
    const ranges:{start:number;end:number;id:string}[]=[];
    let offset=0;
    for(const segment of result.segments) {
      if(!segment.source.length)throw new Error("Empty translation segment.");
      if(segment.kind==="word") {const id=String(ordinal++);glossary.set(id,segment);ranges.push({start:offset,end:offset+segment.source.length,id});}
      offset+=segment.source.length;
    }
    const walker=document.createTreeWalker(paragraph,NodeFilter.SHOW_TEXT),nodes:Text[]=[];
    while(walker.nextNode())nodes.push(walker.currentNode as Text);
    let base=0;
    for(const node of nodes) {
      const text=node.data, end=base+text.length,fragment=document.createDocumentFragment();
      let cursor=0;
      for(const range of ranges) {
        const start=Math.max(base,range.start),stop=Math.min(end,range.end);
        if(start>=stop)continue;
        fragment.append(document.createTextNode(text.slice(cursor,start-base)));
        const span=document.createElement("span");
        span.dataset.translationWord=range.id;span.className="translated-word";span.tabIndex=0;span.setAttribute("role","button");
        span.setAttribute("aria-label",`${glossary.get(range.id)!.source}: ${glossary.get(range.id)!.pinyin??""} ${glossary.get(range.id)!.translation}`);
        span.setAttribute("aria-describedby","paragraph-word-card");
        span.textContent=text.slice(start-base,stop-base);fragment.append(span);cursor=stop-base;
      }
      fragment.append(document.createTextNode(text.slice(cursor)));node.replaceWith(fragment);base=end;
    }
  }
  return glossary;
}
