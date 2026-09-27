import {useLayoutEffect,useRef,useState} from "preact/hooks";
import type {ChannelStatus,GlossaryClient} from "../api/glossary";
import {AutofillResistantField} from "../ui/fields";
import "./glossary.css";

export function GlossarySettings({client,workspace}:{client:GlossaryClient;workspace:string}){
  const [status,setStatus]=useState<ChannelStatus|null>(null),[token,setToken]=useState(""),[pending,setPending]=useState(false),[message,setMessage]=useState("");
  const locked=useRef(false),revision=useRef(0);
  useLayoutEffect(()=>{const epoch=++revision.current;setStatus(null);setToken("");setMessage("");locked.current=false;setPending(false);client.status(workspace).then(s=>{if(epoch===revision.current)setStatus(s);}).catch(e=>{if(epoch===revision.current)setMessage(e.message);});return()=>{revision.current++;};},[client,workspace]);
  const run=async(connect:boolean)=>{
    if(locked.current)return;locked.current=true;setPending(true);setMessage("");const epoch=revision.current;
    try{const result=await(connect?client.configure(workspace,token):client.sync(workspace));if(epoch===revision.current){setStatus(result);if(connect)setToken("");setMessage(connect?"Бот подключён":"Проверка публичной истории поставлена в очередь");}}
    catch(e){if(epoch===revision.current)setMessage((e as Error).message);}
    finally{if(epoch===revision.current){locked.current=false;setPending(false);}}
  };
  return <section class="glossary-settings"><h3>Словарь @reading_data_news</h3><div class="glossary-settings-status">{status?<>{status.posts} постов · {status.definitions} определений<br/>{status.configured?`Бот @${status.botUsername}`:"Создайте отдельного бота через @BotFather и добавьте его в канал. Исторические определения доступны и без бота."}<br/>{status.lastPoll?`Последний опрос: ${status.lastPoll}`:"Свежие события бота ещё не получены."}<br/>{status.unindexed} не разобрано · {status.conflicts} конфликтов<br/>{status.pollError||status.historyError||status.coverageNote}</>:"Читаем состояние словаря…"}</div><label>Токен отдельного Telegram-бота<AutofillResistantField type="password" value={token} disabled={pending} onInput={e=>setToken(e.currentTarget.value)}/></label><div class="glossary-settings-actions"><button class="settings-action" aria-label="Подключить бота" disabled={pending||!token||!status?.generationAllowed} aria-busy={pending} onClick={()=>void run(true)}>{pending?<span class="spinner"/>:"Подключить бота"}</button><button class="settings-action" disabled={pending||!status?.indexReady||status.syncPending} onClick={()=>void run(false)}>Проверить историю</button></div><div class="glossary-settings-feedback" role="status">{message}</div></section>;
}
