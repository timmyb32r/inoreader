use crate::{enqueue_work, source_origin, ProductionYdbTransport, YdbTransport};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::*;
use reader_ingest::*;
use std::sync::Arc;
use ydb::{Transaction, Value};

pub struct YdbIngestStore {
    transport: Arc<ProductionYdbTransport>,
    poll_interval: chrono::Duration,
    per_origin_concurrency: usize,
    max_retry_age: chrono::Duration,
}
impl YdbIngestStore {
    pub fn new(
        transport: Arc<ProductionYdbTransport>,
        poll_interval: chrono::Duration,
        per_origin_concurrency: usize,
        max_retry_age: chrono::Duration,
    ) -> Result<Self, StoreError> {
        if per_origin_concurrency == 0 || max_retry_age <= chrono::Duration::zero() {
            return Err(StoreError::Unavailable(
                "scheduler limits must be positive".into(),
            ));
        }
        Ok(Self {
            transport,
            poll_interval,
            per_origin_concurrency,
            max_retry_age,
        })
    }
}
struct RecordBatch(Vec<PolledRecord>);
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct SourcePollState {
    pub(crate) validators: CacheValidators,
    pub(crate) incomplete: bool,
    #[serde(default)]
    pub(crate) last_success_ms: Option<i64>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct SourceHealth {
    pub(crate) last_success_ms: Option<i64>,
    pub(crate) incomplete: bool,
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) last_error_ms: Option<i64>,
    #[serde(default)]
    pub(crate) consecutive_failures: u32,
}
impl<'a, 'b: 'a> IntoIterator for &'a &'b mut RecordBatch {
    type Item = &'a PolledRecord;
    type IntoIter = std::slice::Iter<'a, PolledRecord>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
pub(crate) fn retry_age_exceeded(first_attempt_ms: i64, oldest_allowed_ms: i64) -> bool {
    first_attempt_ms < oldest_allowed_ms
}
pub(crate) fn origin_has_capacity(active: usize, limit: usize) -> bool {
    active < limit
}
pub(crate) fn is_recurring(item: &WorkItem) -> bool {
    matches!(
        item,
        WorkItem::PollSource { .. } | WorkItem::CollectWebFeed { .. }
    )
}
pub(crate) fn manifest_is_current_or_newer(
    current: &ContentManifestPointer,
    candidate: &ContentRevision,
) -> bool {
    current.source_revision > candidate.source_revision
        || (current.source_revision == candidate.source_revision
            && current.fetched_at >= candidate.fetched_at)
}

async fn record_subscription_activity(
    tx: &mut Transaction,
    source: SourceId,
    successful: bool,
    duration_ms: Option<u64>,
    discovered_items: Option<usize>,
    diagnostic: Option<String>,
    occurred_at: DateTime<Utc>,
) -> Result<(), ydb::YdbOrCustomerError> {
    let cutoff = (occurred_at - chrono::Duration::days(30)).timestamp_millis();
    loop {
        let expired = tx
            .query_result_set("SELECT subscription_id,occurred_at_ms,id FROM subscription_activity VIEW expired_activity WHERE occurred_at_ms < $cutoff LIMIT 100")
            .param("$cutoff", cutoff)
            .await?;
        let mut deleted = 0usize;
        for mut expired in expired {
            deleted += 1;
            let subscription: String = expired
                .remove_field_by_name("subscription_id")?
                .try_into()?;
            let occurred: i64 = expired.remove_field_by_name("occurred_at_ms")?.try_into()?;
            let id: String = expired.remove_field_by_name("id")?.try_into()?;
            tx.exec("DELETE FROM subscription_activity WHERE subscription_id=$subscription AND occurred_at_ms=$occurred AND id=$id")
                .param("$subscription", subscription)
                .param("$occurred", occurred)
                .param("$id", id)
                .await?;
        }
        if crate::activity_retention_complete(deleted) {
            break;
        }
    }
    let occurred_at_ms = occurred_at.timestamp_millis();
    for mut row in tx
        .query_result_set(
            "SELECT subscription_id FROM subscription_sources WHERE source_id=$source",
        )
        .param("$source", source.as_uuid().to_string())
        .await?
    {
        let subscription: String = row.remove_field_by_name("subscription_id")?.try_into()?;
        let mut subscription_row = tx
            .query_row("SELECT document FROM subscriptions WHERE id=$id")
            .param("$id", subscription.clone())
            .await?;
        let subscription_document: String = subscription_row
            .remove_field_by_name("document")?
            .try_into()?;
        let subscription_value: Subscription =
            serde_json::from_str(&subscription_document).map_err(customer)?;
        if !matches!(subscription_value.status(), SubscriptionStatus::Active) {
            continue;
        }
        let mut workspace_row = tx
            .query_row("SELECT document FROM workspaces WHERE id=$id")
            .param(
                "$id",
                subscription_value.workspace_id().as_uuid().to_string(),
            )
            .await?;
        let workspace_document: String =
            workspace_row.remove_field_by_name("document")?.try_into()?;
        let workspace: Workspace = serde_json::from_str(&workspace_document).map_err(customer)?;
        if !workspace.accepts_delivery() {
            continue;
        }
        let event = reader_application::SubscriptionActivity {
            id: uuid::Uuid::new_v4(),
            subscription_id: SubscriptionId::from_uuid(
                uuid::Uuid::parse_str(&subscription).map_err(customer)?,
            ),
            occurred_at,
            successful,
            duration_ms,
            discovered_items,
            diagnostic: diagnostic.clone(),
        };
        tx.exec("UPSERT INTO subscription_activity (subscription_id,occurred_at_ms,id,document) VALUES ($subscription,$occurred,$id,$document)")
            .param("$subscription", subscription.clone())
            .param("$occurred", occurred_at_ms)
            .param("$id", event.id.to_string())
            .param("$document", serde_json::to_string(&event).map_err(customer)?)
            .await?;
        tx.exec("DELETE FROM subscription_activity WHERE subscription_id=$subscription AND occurred_at_ms < $cutoff")
            .param("$subscription", subscription)
            .param("$cutoff", cutoff)
            .await?;
    }
    Ok(())
}

impl YdbIngestStore {
    async fn lease_update(
        &self,
        job: JobId,
        token: LeaseToken,
        status: &str,
        deadline: Option<i64>,
        run_at: Option<i64>,
        diagnostic: Option<&str>,
    ) -> Result<(), StoreError> {
        let id = job.as_uuid().to_string();
        let token = token.as_uuid().to_string();
        let status = status.to_owned();
        let diagnostic = diagnostic.map(str::to_owned);
        let changed=self.transport.client.query_client().retry_tx(ydb::closure!([id,token,status,diagnostic],async |tx:&mut Transaction|{
   let row=tx.query_row("SELECT lease_token,revision,item FROM ingest_jobs WHERE id=$id AND status='leased'").param("$id",id.clone()).optional().await?;let Some(mut row)=row else{return Ok::<_,ydb::YdbOrCustomerError>(false)};let current:Option<String>=row.remove_field_by_name("lease_token")?.try_into()?;let revision:u64=row.remove_field_by_name("revision")?.try_into()?;let item:String=row.remove_field_by_name("item")?.try_into()?;if current.as_deref()!=Some(token.as_str()){return Ok(false)}
   tx.exec("UPDATE ingest_jobs SET status=$status,lease_deadline_ms=$deadline,run_at_ms=COALESCE($run_at,run_at_ms),diagnostic=$diagnostic,attempt=CASE WHEN $status='ready' THEN attempt+$one ELSE attempt END,revision=$next WHERE id=$id AND revision=$revision").param("$status",status.clone()).param("$deadline",deadline).param("$run_at",run_at).param("$diagnostic",diagnostic.clone()).param("$one",1_u32).param("$next",revision+1).param("$id",id.clone()).param("$revision",revision).await?;if let Some(diagnostic)=diagnostic.clone(){let item:WorkItem=serde_json::from_str(&item).map_err(customer)?;if let WorkItem::PollSource{source_id}|WorkItem::RefreshSource{source_id}|WorkItem::CollectWebFeed{source_id}=item{let key=source_id.as_uuid().to_string();let current=tx.query_row("SELECT document FROM source_health WHERE source_id=$id").param("$id",key.clone()).optional().await?;let mut health=current.map(|mut row|->Result<SourceHealth,ydb::YdbOrCustomerError>{let raw:String=row.remove_field_by_name("document")?.try_into()?;serde_json::from_str(&raw).map_err(customer)}).transpose()?.unwrap_or(SourceHealth{last_success_ms:None,incomplete:false,error:None,last_error_ms:None,consecutive_failures:0});health.error=Some(diagnostic.clone());health.last_error_ms=Some(Utc::now().timestamp_millis());health.consecutive_failures=health.consecutive_failures.saturating_add(1);tx.exec("UPSERT INTO source_health (source_id,document) VALUES ($id,$document)").param("$id",key).param("$document",serde_json::to_string(&health).map_err(customer)?).await?;record_subscription_activity(tx,source_id,false,None,None,Some(diagnostic),Utc::now()).await?;}}Ok(true)
  })).await.map_err(storage)?;
        if changed {
            Ok(())
        } else {
            Err(StoreError::StaleLease)
        }
    }
    async fn commit_poll_tx(
        &self,
        lease: &LeasedWork,
        commit: PollCommit,
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        let incomplete = commit.incomplete;
        let source_id = commit.source_id;
        let observed_at_ms = commit.fetched_at.timestamp_millis();
        let duration_ms = commit.duration_ms;
        let discovered_items = commit.records.len();
        let records = commit.records;
        let validators = encode(&SourcePollState {
            validators: commit.validators,
            incomplete,
            last_success_ms: Some(observed_at_ms),
        })?;
        let state_key = format!("source/{}", source_id.as_uuid());
        let records = RecordBatch(records);
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,records,validators,state_key],async |tx:&mut Transaction|{
   assert_lease(tx,&job,&token).await?;
   if !records.0.is_empty(){
    let source_origin_key=source_origin(tx,source_id).await?;
    let mut source_rows=Vec::with_capacity(records.0.len());
    let mut identity_rows=Vec::with_capacity(records.0.len());
    let mut job_rows=Vec::with_capacity(records.0.len().saturating_mul(2));
    let first_attempt_ms=Utc::now().timestamp_millis();
    for record in &records{
     let document=serde_json::to_string(record).map_err(customer)?;
     source_rows.push(ydb::ydb_struct!("id"=>record.id().as_uuid().to_string(),"revision"=>record.revision(),"document"=>document.clone()));
     identity_rows.push(ydb::ydb_struct!("source_id"=>record.source_id().as_uuid().to_string(),"upstream_id"=>record.upstream_id().to_owned(),"record_id"=>record.id().as_uuid().to_string(),"observed_at_ms"=>observed_at_ms,"revision"=>record.revision(),"document"=>document));
     if matches!(record.action,PollAction::Deliver|PollAction::Regroup){
      let item=WorkItem::FanOut{source_id:record.source_id(),record_id:record.id(),after_subscription:None};
      job_rows.push(ydb::ydb_struct!("id"=>record_job("fanout",record.id(),record.revision()).as_uuid().to_string(),"status"=>"ready".to_owned(),"run_at_ms"=>0_i64,"first_attempt_ms"=>first_attempt_ms,"origin_key"=>source_origin_key.clone(),"attempt"=>0_u32,"item"=>serde_json::to_string(&item).map_err(customer)?,"revision"=>0_u64));
     }
     if let Some(url)=record.key().location.fetch_url(){
      let item=WorkItem::ExtractFullText{record_id:record.id(),source_revision:record.revision(),url:url.clone(),manual:false};
      job_rows.push(ydb::ydb_struct!("id"=>record_job("fulltext",record.id(),record.revision()).as_uuid().to_string(),"status"=>"ready".to_owned(),"run_at_ms"=>0_i64,"first_attempt_ms"=>first_attempt_ms,"origin_key"=>crate::http_origin(url)?,"attempt"=>0_u32,"item"=>serde_json::to_string(&item).map_err(customer)?,"revision"=>0_u64));
     }
    }
    let source_values=Value::list_from(ydb::ydb_struct!("id"=>String::new(),"revision"=>0_u64,"document"=>String::new()),source_rows).map_err(customer)?;
    tx.exec("UPSERT INTO source_records SELECT id,revision,document FROM AS_TABLE($rows)").param("$rows",source_values).await?;
    let identity_values=Value::list_from(ydb::ydb_struct!("source_id"=>String::new(),"upstream_id"=>String::new(),"record_id"=>String::new(),"observed_at_ms"=>0_i64,"revision"=>0_u64,"document"=>String::new()),identity_rows).map_err(customer)?;
    tx.exec("UPSERT INTO source_record_identity SELECT source_id,upstream_id,record_id,observed_at_ms,revision,document FROM AS_TABLE($rows)").param("$rows",identity_values).await?;
    if !job_rows.is_empty(){
     let job_values=Value::list_from(ydb::ydb_struct!("id"=>String::new(),"status"=>String::new(),"run_at_ms"=>0_i64,"first_attempt_ms"=>0_i64,"origin_key"=>String::new(),"attempt"=>0_u32,"item"=>String::new(),"revision"=>0_u64),job_rows).map_err(customer)?;
     let existing=tx.query_result_set("SELECT j.id,j.item FROM ingest_jobs AS j INNER JOIN AS_TABLE($rows) AS incoming ON j.id=incoming.id").param("$rows",job_values.clone()).await?;
     for mut row in existing{let id:String=row.remove_field_by_name("id")?.try_into()?;let item:String=row.remove_field_by_name("item")?.try_into()?;let incoming=job_values.clone();let mut matched=tx.query_row("SELECT item FROM AS_TABLE($rows) WHERE id=$id").param("$rows",incoming).param("$id",id).await?;let expected:String=matched.remove_field_by_name("item")?.try_into()?;if item!=expected{return Err(ydb::YdbOrCustomerError::from_err(std::io::Error::other("durable job identity collision")))}}
     tx.exec("UPSERT INTO ingest_jobs SELECT id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision FROM AS_TABLE($rows)").param("$rows",job_values).await?;
    }
   }
   tx.exec("UPSERT INTO content_refresh_state (id,revision,document) VALUES ($id,0,$document)").param("$id",state_key.clone()).param("$document",validators.clone()).await?;let health_key=source_id.as_uuid().to_string();let prior=tx.query_row("SELECT document FROM source_health WHERE source_id=$id").param("$id",health_key.clone()).optional().await?;let last_error_ms=prior.map(|mut row|->Result<SourceHealth,ydb::YdbOrCustomerError>{let raw:String=row.remove_field_by_name("document")?.try_into()?;serde_json::from_str(&raw).map_err(customer)}).transpose()?.and_then(|health|health.last_error_ms);let health=SourceHealth{last_success_ms:Some(observed_at_ms),incomplete,error:None,last_error_ms,consecutive_failures:0};tx.exec("UPSERT INTO source_health (source_id,document) VALUES ($id,$document)").param("$id",source_id.as_uuid().to_string()).param("$document",serde_json::to_string(&health).map_err(customer)?).await?;record_subscription_activity(tx,source_id,true,Some(duration_ms),Some(discovered_items),None,Utc::now()).await?;if incomplete{let item=WorkItem::RefreshSource{source_id};enqueue_work(tx,source_job(source_id,9).as_uuid().to_string(),&item,Utc::now().timestamp_millis()).await?;}Ok::<(),ydb::YdbOrCustomerError>(())
  })).await.map_err(storage)
    }
    async fn record_source_success_tx(
        &self,
        lease: &LeasedWork,
        source: SourceId,
        at: DateTime<Utc>,
        incomplete: bool,
        duration_ms: u64,
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        self.transport
            .client
            .query_client()
            .retry_tx(ydb::closure!([job, token], async |tx: &mut Transaction| {
                assert_lease(tx, &job, &token).await?;
                let key = source.as_uuid().to_string();
                let prior = tx
                    .query_row("SELECT document FROM source_health WHERE source_id=$id")
                    .param("$id", key.clone())
                    .optional()
                    .await?;
                let last_error_ms = prior
                    .map(|mut row| -> Result<SourceHealth, ydb::YdbOrCustomerError> {
                        let raw: String = row.remove_field_by_name("document")?.try_into()?;
                        serde_json::from_str(&raw).map_err(customer)
                    })
                    .transpose()?
                    .and_then(|health| health.last_error_ms);
                let health = SourceHealth {
                    last_success_ms: Some(at.timestamp_millis()),
                    incomplete,
                    error: None,
                    last_error_ms,
                    consecutive_failures: 0,
                };
                tx.exec("UPSERT INTO source_health (source_id,document) VALUES ($id,$document)")
                    .param("$id", source.as_uuid().to_string())
                    .param(
                        "$document",
                        serde_json::to_string(&health).map_err(customer)?,
                    )
                    .await?;
                record_subscription_activity(tx, source, true, Some(duration_ms), None, None, at)
                    .await?;
                Ok::<(), ydb::YdbOrCustomerError>(())
            }))
            .await
            .map_err(storage)
    }
    async fn deliver_tx(
        &self,
        lease: &LeasedWork,
        commit: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        let workspace = commit.target.workspace_id.as_uuid().to_string();
        let subscription = commit.target.subscription_id.as_uuid().to_string();
        let proposed = commit.proposed_article_id;
        let record = commit.record;
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,workspace,subscription,record],async |tx:&mut Transaction|{
   assert_lease(tx,&job,&token).await?;
   let mut sub=tx.query_row("SELECT document FROM subscriptions WHERE id=$id").param("$id",subscription.clone()).await?;
   let sub_doc:String=sub.remove_field_by_name("document")?.try_into()?;
   let sub_value:Subscription=serde_json::from_str(&sub_doc).map_err(customer)?;
   let mut workspace_row=tx.query_row("SELECT document FROM workspaces WHERE id=$id").param("$id",workspace.clone()).await?;
   let workspace_doc:String=workspace_row.remove_field_by_name("document")?.try_into()?;
   let workspace_value:Workspace=serde_json::from_str(&workspace_doc).map_err(customer)?;
   if !matches!(sub_value.status(),SubscriptionStatus::Active)||!workspace_value.accepts_delivery(){return Ok::<_,ydb::YdbOrCustomerError>(DeliveryResult::SkippedInactive)}
   let prior_rows=tx.query_result_set("SELECT article_id,subscription_id FROM library_origins WHERE workspace_id=$workspace AND source_record_id=$record").param("$workspace",workspace.clone()).param("$record",record.id().as_uuid().to_string()).await?;
   let mut prior_by_article=std::collections::BTreeMap::<String,Vec<String>>::new();
   for mut origin in prior_rows{let old_id:String=origin.remove_field_by_name("article_id")?.try_into()?;let old_subscription:String=origin.remove_field_by_name("subscription_id")?.try_into()?;prior_by_article.entry(old_id).or_default().push(old_subscription);}
   let mut inherited=Vec::new();let mut moved_articles=Vec::new();let mut moved_subscriptions=Vec::new();
   for(old_id,subscriptions)in prior_by_article{let old_key=format!("{workspace}/{old_id}");let Some(mut old_row)=tx.query_row("SELECT document FROM articles WHERE id=$id").param("$id",old_key.clone()).optional().await? else{return Err(customer(std::io::Error::other("library origin references a missing article")))};let old_doc:String=old_row.remove_field_by_name("document")?.try_into()?;let mut old:Article=serde_json::from_str(&old_doc).map_err(customer)?;if old.key==*record.key(){continue}moved_articles.push(old.id);inherited.push((old.state,old.first_arrived_at));old.detach_origin(record.id());tx.exec("DELETE FROM library_origins WHERE workspace_id=$workspace AND article_id=$article AND source_record_id=$record").param("$workspace",workspace.clone()).param("$article",old_id.clone()).param("$record",record.id().as_uuid().to_string()).await?;for subscription in subscriptions{if !moved_subscriptions.contains(&subscription){moved_subscriptions.push(subscription)}}let old_dedup=serde_json::to_string(&old.key).map_err(customer)?;if old.origins.is_empty(){tx.exec("DELETE FROM articles WHERE id=$id").param("$id",old_key).await?;tx.exec("DELETE FROM library_dedup WHERE workspace_id=$workspace AND dedup_key=$dedup").param("$workspace",workspace.clone()).param("$dedup",old_dedup).await?}else{let document=serde_json::to_string(&old).map_err(customer)?;tx.exec("UPSERT INTO articles (id,revision,document) VALUES ($id,$revision,$document)").param("$id",old_key).param("$revision",old.revision).param("$document",document.clone()).await?;tx.exec("UPSERT INTO library_dedup (workspace_id,dedup_key,article_id,revision,document) VALUES ($workspace,$dedup,$article,$revision,$document)").param("$workspace",workspace.clone()).param("$dedup",old_dedup).param("$article",old.id.as_uuid().to_string()).param("$revision",old.revision).param("$document",document).await?}}
   let dedup_key=serde_json::to_string(record.key()).map_err(customer)?;
   let found=tx.query_row("SELECT document FROM library_dedup WHERE workspace_id=$workspace AND dedup_key=$dedup").param("$workspace",workspace.clone()).param("$dedup",dedup_key.clone()).optional().await?.map(|mut row|->Result<Article,ydb::YdbOrCustomerError>{let doc:String=row.remove_field_by_name("document")?.try_into()?;serde_json::from_str(&doc).map_err(customer)}).transpose()?;
   let inherited_time=inherited.iter().map(|(_,time)|*time).min();let inherited=ArticleState::merge(inherited.into_iter().map(|(state,_)|state));let found=match(found,inherited){(Some(mut article),Some(state))=>{article.state=ArticleState::merge([article.state,state]).expect("two states");if let Some(time)=inherited_time{article.first_arrived_at=article.first_arrived_at.min(time)}Some(article)},(Some(article),None)=>Some(article),(None,Some(state))=>Some(Article{id:proposed,key:record.key().clone(),state,first_arrived_at:inherited_time.unwrap_or(commit.delivered_at),origins:vec![],revision:0}),(None,None)=>None};
   let (mut article,result)=match found{Some(value)=>{let id=value.id;(value,DeliveryResult::AlreadyDelivered(id))},None=>(Article{id:proposed,key:record.key().clone(),state:ArticleState::default(),first_arrived_at:commit.delivered_at,origins:vec![],revision:0},DeliveryResult::Delivered(proposed))};article.attach_origin(record.id());let document=serde_json::to_string(&article).map_err(customer)?;let key=format!("{workspace}/{}",article.id.as_uuid());tx.exec("UPSERT INTO articles (id,revision,document) VALUES ($id,$revision,$document)").param("$id",key).param("$revision",article.revision).param("$document",document.clone()).await?;tx.exec("UPSERT INTO library_dedup (workspace_id,dedup_key,article_id,revision,document) VALUES ($workspace,$dedup,$article,$revision,$document)").param("$workspace",workspace.clone()).param("$dedup",dedup_key).param("$article",article.id.as_uuid().to_string()).param("$revision",article.revision).param("$document",document).await?;
   if !moved_subscriptions.contains(&subscription){moved_subscriptions.push(subscription.clone())}for linked_subscription in &moved_subscriptions{tx.exec("UPSERT INTO library_origins (workspace_id,article_id,subscription_id,source_record_id) VALUES ($workspace,$article,$subscription,$record)").param("$workspace",workspace.clone()).param("$article",article.id.as_uuid().to_string()).param("$subscription",linked_subscription.clone()).param("$record",record.id().as_uuid().to_string()).await?;}
   for old_article in moved_articles{if old_article==article.id{continue}let rows=tx.query_result_set("SELECT rule_id,rule_version,document FROM rule_evaluations WHERE workspace_id=$workspace AND article_id=$article").param("$workspace",workspace.clone()).param("$article",old_article.as_uuid().to_string()).await?;for mut row in rows{let rule_id:String=row.remove_field_by_name("rule_id")?.try_into()?;let version:u64=row.remove_field_by_name("rule_version")?.try_into()?;let old_document:String=row.remove_field_by_name("document")?.try_into()?;let existing=tx.query_row("SELECT document FROM rule_evaluations WHERE workspace_id=$workspace AND article_id=$article AND rule_id=$rule AND rule_version=$version").param("$workspace",workspace.clone()).param("$article",article.id.as_uuid().to_string()).param("$rule",rule_id.clone()).param("$version",version).optional().await?;let document=if let Some(mut existing)=existing{let current:String=existing.remove_field_by_name("document")?.try_into()?;serde_json::to_string(&serde_json::json!({"merged_provenance":[serde_json::from_str::<serde_json::Value>(&current).map_err(customer)?,serde_json::from_str::<serde_json::Value>(&old_document).map_err(customer)?]})).map_err(customer)?}else{old_document};tx.exec("UPSERT INTO rule_evaluations (workspace_id,article_id,rule_id,rule_version,document) VALUES ($workspace,$article,$rule,$version,$document)").param("$workspace",workspace.clone()).param("$article",article.id.as_uuid().to_string()).param("$rule",rule_id.clone()).param("$version",version).param("$document",document).await?;tx.exec("DELETE FROM rule_evaluations WHERE workspace_id=$workspace AND article_id=$article AND rule_id=$rule AND rule_version=$version").param("$workspace",workspace.clone()).param("$article",old_article.as_uuid().to_string()).param("$rule",rule_id).param("$version",version).await?;}}
   let rule_prefix=format!("{workspace}/");let rule_upper=format!("{workspace}0");let rule_rows=tx.query_result_set("SELECT document FROM rules WHERE id >= $prefix AND id < $upper").param("$prefix",rule_prefix).param("$upper",rule_upper).await?;let mut rules=Vec::new();for mut row in rule_rows{let rule_doc:String=row.remove_field_by_name("document")?.try_into()?;let rule:Rule=serde_json::from_str(&rule_doc).map_err(customer)?;rule.validate().map_err(customer)?;if rule.enabled{rules.push(rule)}}
   for linked_subscription in moved_subscriptions{let linked=SubscriptionId::from_uuid(uuid::Uuid::parse_str(&linked_subscription).map_err(customer)?);let evaluations=rules.iter().filter(|rule|rule.subscription_id==linked).map(|rule|PendingRuleEvaluation{rule_id:rule.id,rule_version:rule.version,subscription_id:rule.subscription_id}).collect::<Vec<_>>();if !evaluations.is_empty(){let item=WorkItem::EvaluateArticleRules{workspace_id:sub_value.workspace_id(),article_id:article.id,evaluations};enqueue_work(tx,article_rule_job(article.id,linked,record.id(),record.revision()).as_uuid().to_string(),&item,Utc::now().timestamp_millis()).await?;}}
   Ok(result)
  })).await.map_err(storage)
    }
    async fn enqueue_continuation(
        &self,
        lease: &LeasedWork,
        after: SubscriptionId,
    ) -> Result<(), StoreError> {
        let WorkItem::FanOut {
            source_id,
            record_id,
            ..
        } = lease.item
        else {
            return Ok(());
        };
        let item = WorkItem::FanOut {
            source_id,
            record_id,
            after_subscription: Some(after),
        };
        let id = durable_job_id(&format!(
            "fanout-continuation/{}/{}",
            lease.job_id.as_uuid(),
            after.as_uuid()
        ))
        .as_uuid()
        .to_string();
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        self.transport
            .client
            .query_client()
            .retry_tx(ydb::closure!(
                [id, item, job, token],
                async |tx: &mut Transaction| {
                    assert_lease(tx, &job, &token).await?;
                    enqueue_work(tx, id.clone(), &item, Utc::now().timestamp_millis()).await
                }
            ))
            .await
            .map_err(storage)
    }
    async fn publish_content_tx(
        &self,
        lease: &LeasedWork,
        revision: ContentRevision,
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,revision],async |tx:&mut Transaction|{
   assert_lease(tx,&job,&token).await?;
   let current=tx.query_row("SELECT document FROM content_manifests WHERE id=$id").param("$id",revision.record_id.as_uuid().to_string()).optional().await?;let current=current.map(|mut row|->Result<ContentManifestPointer,ydb::YdbOrCustomerError>{let document:String=row.remove_field_by_name("document")?.try_into()?;serde_json::from_str(&document).map_err(customer)}).transpose()?;if current.as_ref().is_some_and(|value|manifest_is_current_or_newer(value,&revision)){return Ok::<(),ydb::YdbOrCustomerError>(())}
   for (kind,chunks) in [("raw",&revision.raw_chunks),("safe",&revision.safe_html_chunks)]{for chunk in chunks{let bytes=serde_json::to_string(&chunk.bytes).map_err(customer)?;tx.exec("UPSERT INTO staged_content_chunks (record_id,refresh_id,representation,ordinal,bytes) VALUES ($record,$refresh,$kind,$ordinal,$bytes)").param("$record",revision.record_id.as_uuid().to_string()).param("$refresh",revision.refresh_id.to_string()).param("$kind",kind).param("$ordinal",chunk.ordinal).param("$bytes",bytes).await?}}
   let pointer=ContentManifestPointer::from(&revision);tx.exec("UPSERT INTO content_manifests (id,revision,document) VALUES ($id,$revision,$document)").param("$id",revision.record_id.as_uuid().to_string()).param("$revision",revision.source_revision).param("$document",serde_json::to_string(&pointer).map_err(customer)?).await?;tx.exec("DELETE FROM content_refresh_state WHERE id=$id").param("$id",format!("failure/{}",revision.record_id.as_uuid())).await?;let cleanup=WorkItem::CleanupContent{record_id:revision.record_id,keep_refresh_id:revision.refresh_id};enqueue_work(tx,cleanup_job(revision.record_id,revision.refresh_id).as_uuid().to_string(),&cleanup,Utc::now().timestamp_millis()).await?;Ok::<(),ydb::YdbOrCustomerError>(())
  })).await.map_err(storage)
    }
    async fn finish(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError> {
        let id = job.as_uuid().to_string();
        let token = token.as_uuid().to_string();
        let next_run = millis(Utc::now() + self.poll_interval);
        let changed=self.transport.client.query_client().retry_tx(ydb::closure!([id,token],async |tx:&mut Transaction|{let row=tx.query_row("SELECT lease_token,item,revision FROM ingest_jobs WHERE id=$id AND status='leased'").param("$id",id.clone()).optional().await?;let Some(mut row)=row else{return Ok::<_,ydb::YdbOrCustomerError>(false)};let current:Option<String>=row.remove_field_by_name("lease_token")?.try_into()?;let item:String=row.remove_field_by_name("item")?.try_into()?;let revision:u64=row.remove_field_by_name("revision")?.try_into()?;if current.as_deref()!=Some(token.as_str()){return Ok(false)}let item:WorkItem=serde_json::from_str(&item).map_err(customer)?;let recurring=is_recurring(&item);let(status,run_at,first,attempt)=if recurring{("ready",next_run,next_run,0u32)}else{("completed",0,0,0u32)};tx.exec("UPDATE ingest_jobs SET status=$status,run_at_ms=$run_at,first_attempt_ms=CASE WHEN $recurring THEN $first ELSE first_attempt_ms END,attempt=CASE WHEN $recurring THEN $attempt ELSE attempt END,lease_token=NULL,lease_deadline_ms=NULL,revision=$next WHERE id=$id AND revision=$revision").param("$status",status).param("$run_at",run_at).param("$recurring",recurring).param("$first",first).param("$attempt",attempt).param("$next",revision+1).param("$id",id.clone()).param("$revision",revision).await?;Ok(true)})).await.map_err(storage)?;
        if changed {
            Ok(())
        } else {
            Err(StoreError::StaleLease)
        }
    }
}

fn customer(error: impl std::error::Error + 'static + Send + Sync) -> ydb::YdbOrCustomerError {
    ydb::YdbOrCustomerError::from_err(error)
}
async fn assert_lease(
    tx: &mut Transaction,
    job: &str,
    token: &str,
) -> Result<(), ydb::YdbOrCustomerError> {
    let row=tx.query_row("SELECT lease_token,lease_deadline_ms FROM ingest_jobs WHERE id=$id AND status='leased'").param("$id",job.to_owned()).optional().await?;
    let Some(mut row) = row else {
        return Err(customer(std::io::Error::other("stale lease")));
    };
    let stored: Option<String> = row.remove_field_by_name("lease_token")?.try_into()?;
    let deadline: Option<i64> = row.remove_field_by_name("lease_deadline_ms")?.try_into()?;
    if stored.as_deref() != Some(token)
        || deadline.is_none_or(|v| v < Utc::now().timestamp_millis())
    {
        return Err(customer(std::io::Error::other("stale lease")));
    }
    Ok(())
}
async fn article_full_text(
    tx: &mut Transaction,
    article: &Article,
) -> Result<Option<String>, ydb::YdbOrCustomerError> {
    let mut text = String::new();
    for origin in &article.origins {
        let Some(mut manifest_row) = tx
            .query_row("SELECT document FROM content_manifests WHERE id=$id")
            .param("$id", origin.as_uuid().to_string())
            .optional()
            .await?
        else {
            continue;
        };
        let manifest_doc: String = manifest_row.remove_field_by_name("document")?.try_into()?;
        let manifest: ContentManifestPointer =
            serde_json::from_str(&manifest_doc).map_err(customer)?;
        let chunks=tx.query_result_set("SELECT bytes FROM staged_content_chunks WHERE record_id=$record AND refresh_id=$refresh AND representation='safe' ORDER BY ordinal").param("$record",origin.as_uuid().to_string()).param("$refresh",manifest.refresh_id.to_string()).await?;
        for mut chunk in chunks {
            let encoded: String = chunk.remove_field_by_name("bytes")?.try_into()?;
            let bytes: Vec<u8> = serde_json::from_str(&encoded).map_err(customer)?;
            text.push_str(std::str::from_utf8(&bytes).map_err(customer)?);
        }
    }
    Ok((!text.is_empty()).then_some(text))
}

fn storage(error: impl ToString) -> StoreError {
    StoreError::Unavailable(error.to_string())
}
fn decode<T: serde::de::DeserializeOwned>(raw: &[u8]) -> Result<T, StoreError> {
    serde_json::from_slice(raw).map_err(storage)
}
fn encode<T: serde::Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(storage)
}
fn millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}
fn durable_job_id(identity: &str) -> JobId {
    let hash = murmur3::murmur3_x64_128(&mut std::io::Cursor::new(identity.as_bytes()), 0)
        .expect("in-memory hashing cannot fail");
    JobId::from_uuid(uuid::Uuid::from_u128(hash))
}
pub(crate) fn record_job(kind: &str, record: SourceRecordId, revision: u64) -> JobId {
    durable_job_id(&format!("{kind}/{}/{revision}", record.as_uuid()))
}
pub(crate) fn cleanup_job(record: SourceRecordId, refresh: uuid::Uuid) -> JobId {
    durable_job_id(&format!("cleanup/{}/{refresh}", record.as_uuid()))
}
fn source_job(source: SourceId, tag: u8) -> JobId {
    durable_job_id(&format!("source/{}/{tag}", source.as_uuid()))
}
fn rule_job(rule: RuleId, version: u64, cursor: ArticleId) -> JobId {
    durable_job_id(&format!(
        "rule/{}/{version}/{}",
        rule.as_uuid(),
        cursor.as_uuid()
    ))
}
pub(crate) fn article_rule_job(
    article: ArticleId,
    subscription: SubscriptionId,
    record: SourceRecordId,
    revision: u64,
) -> JobId {
    durable_job_id(&format!(
        "article-rule/{}/{}/{}/{revision}",
        article.as_uuid(),
        subscription.as_uuid(),
        record.as_uuid()
    ))
}

#[async_trait]
impl IngestStore for YdbIngestStore {
    async fn claim(
        &self,
        worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<LeasedWork>, StoreError> {
        let worker = worker.to_owned();
        let token = LeaseToken::new();
        let now_ms = millis(now);
        let deadline = millis(lease_until);
        let token_text = token.as_uuid().to_string();
        let oldest_ms = millis(now - self.max_retry_age);
        let origin_limit = self.per_origin_concurrency as u64;
        self.transport.client.query_client().retry_tx(ydb::closure!([worker,token_text],async |tx:&mut Transaction|{
            let _worker = &worker;
            let mut rows: Vec<_> = tx.query_result_set(
                "SELECT id,item,attempt,revision,first_attempt_ms,origin_key FROM ingest_jobs VIEW ready_jobs WHERE status='ready' AND run_at_ms <= $now ORDER BY status,run_at_ms LIMIT 128"
            ).param("$now",now_ms).await?.into_iter().collect();
            if rows.is_empty() {
                rows = tx.query_result_set(
                    "SELECT id,item,attempt,revision,first_attempt_ms,origin_key FROM ingest_jobs WHERE status='leased' AND lease_deadline_ms < $now LIMIT 128"
                ).param("$now",now_ms).await?.into_iter().collect();
            }
            for mut row in rows {
                let id:String=row.remove_field_by_name("id")?.try_into()?;
                let item:String=row.remove_field_by_name("item")?.try_into()?;
                let attempt:u32=row.remove_field_by_name("attempt")?.try_into()?;
                let revision:u64=row.remove_field_by_name("revision")?.try_into()?;
                let first_attempt_ms:i64=row.remove_field_by_name("first_attempt_ms")?.try_into()?;
                let origin_key:String=row.remove_field_by_name("origin_key")?.try_into()?;
                if retry_age_exceeded(first_attempt_ms,oldest_ms) {
                    tx.exec("UPDATE ingest_jobs SET status='failed',lease_token=NULL,lease_deadline_ms=NULL,diagnostic='maximum retry age exceeded',revision=$next WHERE id=$id AND revision=$revision").param("$next",revision+1).param("$id",id).param("$revision",revision).await?;
                    continue;
                }
                let active=tx.query_result_set("SELECT id FROM ingest_jobs VIEW leased_by_origin WHERE status='leased' AND origin_key=$origin AND lease_deadline_ms >= $now LIMIT $limit").param("$origin",origin_key).param("$now",now_ms).param("$limit",origin_limit).await?.into_iter().count();
                if !origin_has_capacity(active,origin_limit as usize) { continue; }
                tx.exec("UPDATE ingest_jobs SET status='leased',lease_token=$token,lease_deadline_ms=$deadline,revision=$next WHERE id=$id AND revision=$revision").param("$token",token_text.clone()).param("$deadline",deadline).param("$next",revision+1).param("$id",id.clone()).param("$revision",revision).await?;
                return Ok(Some((id,item,attempt)));
            }
            Ok::<_,ydb::YdbOrCustomerError>(None)
        })).await.map_err(storage)?.map(|(id,item,attempt)|Ok(LeasedWork{job_id:JobId::from_uuid(uuid::Uuid::parse_str(&id).map_err(storage)?),item:serde_json::from_str(&item).map_err(storage)?,token,deadline:lease_until,attempt})).transpose()
    }
    async fn renew(
        &self,
        job: JobId,
        token: LeaseToken,
        lease_until: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.lease_update(job, token, "leased", Some(millis(lease_until)), None, None)
            .await
    }
    async fn complete(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError> {
        self.finish(job, token).await
    }
    async fn retry(
        &self,
        job: JobId,
        token: LeaseToken,
        run_at: DateTime<Utc>,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        self.lease_update(
            job,
            token,
            "ready",
            None,
            Some(millis(run_at)),
            Some(diagnostic),
        )
        .await
    }
    async fn fail(
        &self,
        job: JobId,
        token: LeaseToken,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        self.lease_update(job, token, "failed", None, None, Some(diagnostic))
            .await
    }
    async fn source(&self, id: SourceId) -> Result<SourceDefinition, StoreError> {
        decode(
            &self
                .transport
                .read("sources", id.as_uuid().to_string())
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?,
        )
    }
    async fn has_committed_poll(&self, source: SourceId) -> Result<bool, StoreError> {
        Ok(self
            .transport
            .read(
                "content_refresh_state",
                format!("source/{}", source.as_uuid()),
            )
            .await
            .map_err(storage)?
            .is_some())
    }
    async fn source_validators(&self, source: SourceId) -> Result<CacheValidators, StoreError> {
        match self
            .transport
            .read(
                "content_refresh_state",
                format!("source/{}", source.as_uuid()),
            )
            .await
            .map_err(storage)?
        {
            Some(v) => decode::<SourcePollState>(&v)
                .map(|state| state.validators)
                .or_else(|_| decode(&v)),
            None => Ok(CacheValidators::default()),
        }
    }
    async fn active_delivery_count(&self, source: SourceId) -> Result<u64, StoreError> {
        let source = self.source(source).await?;
        let docs = self
            .transport
            .scan_prefix("subscriptions", "".into())
            .await
            .map_err(storage)?;
        let mut count = 0;
        for raw in docs {
            let sub: Subscription = decode(&raw)?;
            if sub.source_url() == source.url()
                && matches!(sub.status(), SubscriptionStatus::Active)
            {
                let workspace: Workspace = decode(
                    &self
                        .transport
                        .read("workspaces", sub.workspace_id().as_uuid().to_string())
                        .await
                        .map_err(storage)?
                        .ok_or(StoreError::NotFound)?,
                )?;
                if workspace.accepts_delivery() {
                    count += 1
                }
            }
        }
        Ok(count)
    }
    async fn commit_poll(&self, lease: &LeasedWork, commit: PollCommit) -> Result<(), StoreError> {
        self.commit_poll_tx(lease, commit).await
    }
    async fn record_source_success(
        &self,
        lease: &LeasedWork,
        source: SourceId,
        at: DateTime<Utc>,
        incomplete: bool,
        duration_ms: u64,
    ) -> Result<(), StoreError> {
        self.record_source_success_tx(lease, source, at, incomplete, duration_ms)
            .await
    }
    async fn record(&self, id: SourceRecordId) -> Result<SourceRecord, StoreError> {
        for raw in self
            .transport
            .scan_prefix("source_records", "".into())
            .await
            .map_err(storage)?
        {
            let value: SourceRecord = decode(&raw)?;
            if value.id() == id {
                return Ok(value);
            }
        }
        Err(StoreError::NotFound)
    }
    async fn record_by_upstream(
        &self,
        source: SourceId,
        upstream_id: &str,
    ) -> Result<Option<SourceRecord>, StoreError> {
        let mut query = self.transport.client.query_client();
        let row=query.query_row("SELECT document FROM source_record_identity WHERE source_id=$source AND upstream_id=$upstream").param("$source",source.as_uuid().to_string()).param("$upstream",upstream_id.to_owned()).optional().await.map_err(storage)?;
        match row {
            Some(mut row) => {
                let doc: String = row
                    .remove_field_by_name("document")
                    .map_err(storage)?
                    .try_into()
                    .map_err(storage)?;
                Ok(Some(serde_json::from_str(&doc).map_err(storage)?))
            }
            None => Ok(None),
        }
    }
    async fn delivery_targets(
        &self,
        source: SourceId,
        after: Option<SubscriptionId>,
        limit: usize,
    ) -> Result<Vec<DeliveryTarget>, StoreError> {
        let mut values = Vec::new();
        for raw in self
            .transport
            .scan_prefix("subscriptions", "".into())
            .await
            .map_err(storage)?
        {
            let sub: Subscription = decode(&raw)?;
            let Some(mapped) = self
                .transport
                .source_id_for_subscription(sub.id().as_uuid().to_string())
                .await
                .map_err(storage)?
            else {
                continue;
            };
            if mapped != source.as_uuid().to_string()
                || !matches!(sub.status(), SubscriptionStatus::Active)
                || after.is_some_and(|v| sub.id().as_uuid() <= v.as_uuid())
            {
                continue;
            }
            let workspace: Workspace = decode(
                &self
                    .transport
                    .read("workspaces", sub.workspace_id().as_uuid().to_string())
                    .await
                    .map_err(storage)?
                    .ok_or(StoreError::NotFound)?,
            )?;
            if workspace.accepts_delivery() {
                values.push(DeliveryTarget {
                    workspace_id: sub.workspace_id(),
                    subscription_id: sub.id(),
                })
            }
        }
        values.sort_by_key(|v| v.subscription_id.as_uuid());
        values.truncate(limit);
        Ok(values)
    }
    async fn deliver(
        &self,
        lease: &LeasedWork,
        commit: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError> {
        self.deliver_tx(lease, commit).await
    }
    async fn advance_fanout(
        &self,
        lease: &LeasedWork,
        after: SubscriptionId,
    ) -> Result<(), StoreError> {
        self.enqueue_continuation(lease, after).await
    }
    async fn publish_content(
        &self,
        lease: &LeasedWork,
        revision: ContentRevision,
    ) -> Result<(), StoreError> {
        self.publish_content_tx(lease, revision).await
    }
    async fn cleanup_content(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        _requested_keep: uuid::Uuid,
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token],async |tx:&mut Transaction|{assert_lease(tx,&job,&token).await?;let Some(mut row)=tx.query_row("SELECT document FROM content_manifests WHERE id=$id").param("$id",record.as_uuid().to_string()).optional().await? else{return Err(customer(std::io::Error::other("content cleanup has no current manifest")))};let document:String=row.remove_field_by_name("document")?.try_into()?;let current:ContentManifestPointer=serde_json::from_str(&document).map_err(customer)?;tx.exec("DELETE FROM staged_content_chunks WHERE record_id=$record AND refresh_id != $keep").param("$record",record.as_uuid().to_string()).param("$keep",current.refresh_id.to_string()).await?;Ok::<(),ydb::YdbOrCustomerError>(())})).await.map_err(storage)
    }
    async fn record_refresh_failure(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        let key = format!("failure/{}", record.as_uuid());
        let diagnostic = diagnostic.to_owned();
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,key,diagnostic],async |tx:&mut Transaction|{assert_lease(tx,&job,&token).await?;let current=tx.query_row("SELECT revision FROM content_refresh_state WHERE id=$id").param("$id",key.clone()).optional().await?;let revision=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?.unwrap_or(0).saturating_add(1);let document=serde_json::to_string(&serde_json::json!({"diagnostic":diagnostic,"job":job})).map_err(customer)?;tx.exec("UPSERT INTO content_refresh_state (id,revision,document) VALUES ($id,$revision,$document)").param("$id",key.clone()).param("$revision",revision).param("$document",document).await?;Ok::<(),ydb::YdbOrCustomerError>(())})).await.map_err(storage)
    }
    async fn evaluate_article_rules(
        &self,
        lease: &LeasedWork,
        workspace: WorkspaceId,
        article: ArticleId,
        evaluations: &[PendingRuleEvaluation],
    ) -> Result<(), StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        let workspace = workspace.as_uuid().to_string();
        let article_id = article.as_uuid().to_string();
        let evaluations = evaluations.to_vec();
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,workspace,article_id,evaluations],async |tx:&mut Transaction|{
   assert_lease(tx,&job,&token).await?;let key=format!("{workspace}/{article_id}");let Some(mut row)=tx.query_row("SELECT document FROM articles WHERE id=$id").param("$id",key.clone()).optional().await? else{return Ok::<(),ydb::YdbOrCustomerError>(())};let document:String=row.remove_field_by_name("document")?.try_into()?;let mut article:Article=serde_json::from_str(&document).map_err(customer)?;let original_state=article.state;
   let mut full_text=String::new();let mut content_generations=Vec::new();for origin in &article.origins{let Some(mut manifest_row)=tx.query_row("SELECT document FROM content_manifests WHERE id=$id").param("$id",origin.as_uuid().to_string()).optional().await? else{continue};let manifest_doc:String=manifest_row.remove_field_by_name("document")?.try_into()?;let manifest:ContentManifestPointer=serde_json::from_str(&manifest_doc).map_err(customer)?;let chunks=tx.query_result_set("SELECT bytes FROM staged_content_chunks WHERE record_id=$record AND refresh_id=$refresh AND representation='safe' ORDER BY ordinal").param("$record",origin.as_uuid().to_string()).param("$refresh",manifest.refresh_id.to_string()).await?;for mut chunk in chunks{let encoded:String=chunk.remove_field_by_name("bytes")?.try_into()?;let bytes:Vec<u8>=serde_json::from_str(&encoded).map_err(customer)?;full_text.push_str(std::str::from_utf8(&bytes).map_err(customer)?)}content_generations.push(manifest.refresh_id);}
   let full_text=(!full_text.is_empty()).then_some(full_text);for pending in evaluations.iter(){let rule_key=format!("{workspace}/{}",pending.rule_id.as_uuid());let Some(mut rule_row)=tx.query_row("SELECT document FROM rules WHERE id=$id").param("$id",rule_key).optional().await? else{continue};let rule_doc:String=rule_row.remove_field_by_name("document")?.try_into()?;let rule:Rule=serde_json::from_str(&rule_doc).map_err(customer)?;rule.validate().map_err(customer)?;if !pending.still_valid_for(&rule){continue}let title_match=rule.matches(&article.key.title,None);if full_text.is_none()&&matches!(rule.field,RuleField::Text)||(full_text.is_none()&&matches!(rule.field,RuleField::Both)&&!title_match){return Err(customer(std::io::Error::other("pinned rule evaluation awaits fulltext")))}let matched=rule.matches(&article.key.title,full_text.as_deref());let before=article.state;if matched{rule.apply(&mut article.state)}let reason=serde_json::to_string(&serde_json::json!({"matched":matched,"action":rule.action,"state_changed":before!=article.state,"manual_override_preserved":matched&&before==article.state,"content_generations":content_generations})).map_err(customer)?;tx.exec("UPSERT INTO rule_evaluations (workspace_id,article_id,rule_id,rule_version,document) VALUES ($workspace,$article,$rule,$version,$document)").param("$workspace",workspace.clone()).param("$article",article_id.clone()).param("$rule",rule.id.as_uuid().to_string()).param("$version",rule.version).param("$document",reason).await?;}if article.state!=original_state{article.revision+=1;tx.exec("UPSERT INTO articles (id,revision,document) VALUES ($id,$revision,$document)").param("$id",key).param("$revision",article.revision).param("$document",serde_json::to_string(&article).map_err(customer)?).await?}Ok::<(),ydb::YdbOrCustomerError>(())
  })).await.map_err(storage)
    }
    async fn apply_rule_batch(
        &self,
        lease: &LeasedWork,
        workspace_id: WorkspaceId,
        rule: RuleId,
        version: u64,
        after_id: Option<ArticleId>,
        through_id: Option<ArticleId>,
        limit: usize,
    ) -> Result<Option<ArticleId>, StoreError> {
        let job = lease.job_id.as_uuid().to_string();
        let token = lease.token.as_uuid().to_string();
        let workspace = workspace_id.as_uuid().to_string();
        let rule_id = rule.as_uuid().to_string();
        let after = after_id
            .map(|v| format!("{workspace}/{}", v.as_uuid()))
            .unwrap_or_else(|| format!("{workspace}/"));
        let prefix_upper = format!("{workspace}0");
        self.transport.client.query_client().retry_tx(ydb::closure!([job,token,workspace,rule_id,after,prefix_upper],async |tx:&mut Transaction|{
   assert_lease(tx,&job,&token).await?;let rule_key=format!("{workspace}/{rule_id}");let Some(mut rule_row)=tx.query_row("SELECT document FROM rules WHERE id=$id").param("$id",rule_key).optional().await? else{return Ok::<_,ydb::YdbOrCustomerError>(None)};let rule_doc:String=rule_row.remove_field_by_name("document")?.try_into()?;let rule:Rule=serde_json::from_str(&rule_doc).map_err(customer)?;rule.validate().map_err(customer)?;if !rule.enabled||rule.version!=version{return Ok(None)}
   let through=match through_id{Some(value)=>value,None=>{let Some(mut boundary)=tx.query_row("SELECT id FROM articles WHERE id > $after AND id < $upper ORDER BY id DESC LIMIT 1").param("$after",format!("{workspace}/")).param("$upper",prefix_upper.clone()).optional().await? else{return Ok(None)};let id:String=boundary.remove_field_by_name("id")?.try_into()?;ArticleId::from_uuid(uuid::Uuid::parse_str(id.rsplit('/').next().unwrap_or_default()).map_err(customer)?)} };let through_key=format!("{workspace}/{}",through.as_uuid());
   let rows=tx.query_result_set("SELECT id,document FROM articles WHERE id > $after AND id <= $through ORDER BY id LIMIT $limit").param("$after",after.clone()).param("$through",through_key).param("$limit",limit as u64).await?;let mut count=0;let mut last=None;
   for mut row in rows{count+=1;let id:String=row.remove_field_by_name("id")?.try_into()?;let document:String=row.remove_field_by_name("document")?.try_into()?;let mut article:Article=serde_json::from_str(&document).map_err(customer)?;let before=article.state;let full_text=article_full_text(tx,&article).await?;let title_match=rule.matches(&article.key.title,None);if full_text.is_none()&&(matches!(rule.field,RuleField::Text)||(matches!(rule.field,RuleField::Both)&&!title_match)){return Err(customer(std::io::Error::other("bulk rule evaluation awaits fulltext")))}let matched=rule.matches(&article.key.title,full_text.as_deref());if matched{rule.apply(&mut article.state)}if article.state!=before{article.revision+=1;tx.exec("UPSERT INTO articles (id,revision,document) VALUES ($id,$revision,$document)").param("$id",id).param("$revision",article.revision).param("$document",serde_json::to_string(&article).map_err(customer)?).await?}let reason=serde_json::to_string(&serde_json::json!({"matched":matched,"action":rule.action,"state_changed":before!=article.state,"manual_override_preserved":matched&&before==article.state})).map_err(customer)?;tx.exec("UPSERT INTO rule_evaluations (workspace_id,article_id,rule_id,rule_version,document) VALUES ($workspace,$article,$rule,$version,$document)").param("$workspace",workspace.clone()).param("$article",article.id.as_uuid().to_string()).param("$rule",rule_id.clone()).param("$version",version).param("$document",reason).await?;last=Some(article.id);}
   if count==limit{if let Some(cursor)=last{if cursor!=through{let next=WorkItem::ApplyRule{workspace_id,rule_id:rule.id,rule_version:version,after_article:Some(cursor),through_article:Some(through)};enqueue_work(tx,rule_job(rule.id,version,cursor).as_uuid().to_string(),&next,Utc::now().timestamp_millis()).await?;}}}Ok(last)
  })).await.map_err(storage)
    }
}
