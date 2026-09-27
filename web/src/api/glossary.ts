import type { Transport } from "./client";
export type ChannelStatus = {channel:string;configured:boolean;botUsername:string|null;indexReady:boolean;revision:number;posts:number;definitions:number;unindexed:number;pending:number;conflicts:number;lastPoll:string|null;lastHistory:string|null;pollError:string|null;historyError:string|null;historyIncomplete:boolean;coverageNote:string;syncPending:boolean;generationAllowed:boolean};
export type MarkStyle = {kind:"bold"|"italic"|"underline"|"strike"|"code"|"pre"|"spoiler"|"quote"}|{kind:"link";url:string};
export type StyledText = {text:string;marks:{start:number;end:number;style:MarkStyle}[]};
export type EntityDefinition = {name:string;kind:"company"|"product"|"technology"|"abbreviation"|"protocol";explanation:string;insufficientContext:boolean};
export type KnownDefinition = {definition:{term:string;position:number;paragraph:StyledText};permalink:string;publishedAt:string|null;stale:boolean};
export type DefinitionsJob = {id:string;workspaceId:string;articleId:string;model:string;promptVersion:string} & ({status:"queued"|"generating"}|{status:"completed";result:{entities:EntityDefinition[]}}|{status:"failed";error:string});
export type DefinitionsView = {job:DefinitionsJob|null;channel:ChannelStatus;known:KnownDefinition[]};
const json=(method:string,body:unknown):RequestInit=>({method,headers:{"Content-Type":"application/json"},body:JSON.stringify(body)});
export class GlossaryClient {
  constructor(private readonly transport:Transport){}
  status=(workspace:string)=>this.transport<ChannelStatus>(`/api/glossary/channel?workspace_id=${encodeURIComponent(workspace)}`);
  configure=(workspaceId:string,token:string)=>this.transport<ChannelStatus>("/api/glossary/channel",json("PUT",{workspaceId,token}));
  sync=(workspaceId:string)=>this.transport<ChannelStatus>("/api/glossary/channel/sync",json("POST",{workspaceId}));
  get=(workspace:string,article:string)=>this.transport<DefinitionsView>(`/api/articles/${encodeURIComponent(article)}/definitions?workspace_id=${encodeURIComponent(workspace)}`);
  generate=(workspaceId:string,article:string,operationId:string,regenerate=false)=>this.transport<DefinitionsView>(`/api/articles/${encodeURIComponent(article)}/definitions`,json("POST",{workspaceId,operationId,regenerate}));
}
