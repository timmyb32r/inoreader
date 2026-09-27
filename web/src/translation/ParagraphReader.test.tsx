import {fireEvent,render,screen,waitFor} from "@testing-library/preact";
import {AiClient,type ParagraphJob} from "../api/ai";
import {ParagraphReader} from "./ParagraphReader";
it("immediately shows pending, deduplicates clicks and displays word details without extra requests",async()=>{
 const job:ParagraphJob={id:"job",workspaceId:"ws",articleId:"a",source:"磁盘。",model:"deepseek-flash",status:"completed",result:{source:"磁盘。",translation:"Диск.",segments:[{kind:"word",source:"磁盘",pinyin:"cípán",translation:"диск"},{kind:"literal",source:"。"}]}};
 const client=new AiClient(vi.fn());vi.spyOn(client,"translations").mockResolvedValue([]);
 let finish!:(j:ParagraphJob)=>void;const request=vi.spyOn(client,"translate").mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const {container}=render(<ParagraphReader client={client} workspaceId="ws" articleId="a" html="<p>磁盘。</p>" body={[]} enabled/>);
 await waitFor(()=>expect(container.querySelector('[data-translatable]')).not.toBeNull());
 const p=container.querySelector("p")!;fireEvent.click(p);fireEvent.click(p);
 expect(screen.getByRole("region",{name:"Paragraph translation"})).toHaveAttribute("aria-busy","true");expect(request).toHaveBeenCalledTimes(1);
 finish(job);await screen.findByText("Диск.");
 await waitFor(()=>expect(container.querySelector('[data-translation-word]')).not.toBeNull());
 fireEvent.mouseOver(container.querySelector('[data-translation-word]')!);
 expect(screen.getByRole("tooltip")).toHaveTextContent("cípán");expect(screen.getByRole("tooltip")).toHaveTextContent("диск");expect(request).toHaveBeenCalledTimes(1);
});
it("reuses the operation ID after an uncertain POST failure and keeps source intact",async()=>{
 const client=new AiClient(vi.fn());vi.spyOn(client,"translations").mockResolvedValue([]);
 const request=vi.spyOn(client,"translate").mockRejectedValueOnce(new Error("Connection lost")).mockResolvedValue({id:"job",workspaceId:"ws",articleId:"a",source:"磁盘。",model:"deepseek-flash",status:"failed",error:"Provider rejected request"});
 const {container}=render(<ParagraphReader client={client} workspaceId="ws" articleId="a" html="<p>磁盘。</p>" body={[]} enabled/>);
 await waitFor(()=>expect(container.querySelector('[data-translatable]')).not.toBeNull());
 fireEvent.click(container.querySelector("p")!);
 expect(await screen.findByRole("alert")).toHaveTextContent("Connection lost");
 fireEvent.click(screen.getByRole("button",{name:"Повторить"}));
 await waitFor(()=>expect(request).toHaveBeenCalledTimes(2));
 expect(request.mock.calls[1][2]).toBe(request.mock.calls[0][2]);
 expect(container.querySelector('.article-content')?.textContent).toBe("磁盘。");
 expect(await screen.findByRole("alert")).toHaveTextContent("Provider rejected request");
});
