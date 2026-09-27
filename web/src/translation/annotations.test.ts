import { annotate, clearAnnotations, paragraphs } from "./annotations";
import type { ParagraphTranslation } from "../api/ai";
const result:ParagraphTranslation={source:"磁盘 读取。",translation:"Чтение диска.",segments:[{kind:"word",source:"磁盘",pinyin:"cípán",translation:"диск"},{kind:"literal",source:" "},{kind:"word",source:"读取",pinyin:"dúqǔ",translation:"читать"},{kind:"literal",source:"。"}]};
it("preserves exact text, links and emphasis, including a word across text nodes",()=>{
 const root=document.createElement("div");root.innerHTML='<p>磁<strong>盘</strong> <a href="https://example.com">读取</a>。</p><p><code>code</code></p>';
 const html=root.innerHTML,text=root.textContent;
 expect(paragraphs(root)).toHaveLength(2);
 const glossary=annotate(root,new Map([[result.source,result]]));
 expect(root.textContent).toBe(text);expect(root.querySelector("a")?.getAttribute("href")).toBe("https://example.com");
 expect(glossary.size).toBe(2);
 expect(root.querySelectorAll('[data-translation-word="0"]')).toHaveLength(2);
 clearAnnotations(root);expect(root.innerHTML).toBe(html);
});
it("rejects mismatched segmentation without rewriting source text",()=>{
 const root=document.createElement("div");root.innerHTML='<p>磁盘 读取。</p>';const text=root.textContent;
 expect(()=>annotate(root,new Map([[result.source,{...result,segments:[]}]]))).toThrow("does not match");
 expect(root.textContent).toBe(text);
});
