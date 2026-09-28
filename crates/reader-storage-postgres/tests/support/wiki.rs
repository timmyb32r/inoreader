use super::*;
use reader_storage_postgres::PostgresWikiStore;
use reader_wiki::{ChangeInput, Draft, Error, Limits, LimitsInput, Role, Store, Write, WriteInput};

pub async fn verify(pool: &PgPool) {
    let repository =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let mut actors = Vec::new();
    for index in 0..4 {
        let account = AccountRecord {
            id: AccountId::new(),
            username: format!("wiki-{index}-{}", Uuid::new_v4()),
            password_hash: "fixture".into(),
            admin: index == 3,
            auth_revision: 0,
            revision: 0,
        };
        let workspace = Workspace::new(WorkspaceId::new(), account.id, "Wiki acceptance".into());
        repository
            .create_account_and_workspace(account.clone(), workspace.clone())
            .await
            .unwrap();
        actors.push((account, workspace));
    }
    let a = actors[0].0.id.as_uuid();
    let b = actors[1].0.id.as_uuid();
    let c = actors[2].0.id.as_uuid();
    let admin = actors[3].0.id.as_uuid();
    let limits: Limits = LimitsInput {
        name_bytes: 512,
        markdown_bytes: 524288,
        search_excerpt_characters: 200,
        search_bytes: 512,
        page_size: 2,
        draft_save_delay_ms: 1000,
    }
    .try_into()
    .unwrap();
    let store = std::sync::Arc::new(PostgresWikiStore::new(pool.clone(), limits.clone()));
    let n = Uuid::new_v4();
    let other = Uuid::new_v4();
    store.create_namespace(a, n, "Private").await.unwrap();
    store.create_namespace(b, other, "Private").await.unwrap();
    assert!(matches!(
        store.namespace(admin, n).await,
        Err(Error::NotFound)
    ));
    assert!(store.namespaces(admin, 0).await.unwrap().items.is_empty());
    assert!(matches!(
        store.pages(b, n, "", false, 0).await,
        Err(Error::NotFound)
    ));
    store
        .set_member(a, n, &actors[1].0.username, Some(Role::Reader))
        .await
        .unwrap();
    store
        .set_member(a, n, &actors[2].0.username, Some(Role::Editor))
        .await
        .unwrap();
    assert!(matches!(
        store.set_member(c, n, &actors[1].0.username, None).await,
        Err(Error::Forbidden)
    ));
    let id = Uuid::new_v4();
    let request = WriteInput {
        operation: Uuid::new_v4(),
        page: id,
        expected_revision: None,
        change: ChangeInput::Save {
            name: "Exact 页面 ".into(),
            markdown: "# Original\n\n[[Missing]]\n\nРусский 中文 100% _literal".into(),
        },
    };
    let command = || Write::new(request.clone(), &limits).unwrap();
    assert!(matches!(
        store.write(b, n, command()).await,
        Err(Error::Forbidden)
    ));
    let page = store.write(a, n, command()).await.unwrap();
    assert_eq!(store.links(a, n, id).await.unwrap()[0].name, "Missing");
    assert!(store.links(a, n, id).await.unwrap()[0].page.is_none());
    assert!(matches!(
        store.links(admin, n, id).await,
        Err(Error::NotFound)
    ));
    assert_eq!(
        store.write(a, n, command()).await.unwrap().revision,
        page.revision
    );
    assert_eq!(store.history(a, n, id, 0).await.unwrap().items.len(), 1);
    assert!(matches!(
        store.resolve(a, n, "Missing").await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.page(a, other, id).await,
        Err(Error::NotFound)
    ));
    assert_eq!(
        store
            .pages(a, n, "100%", false, 0)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(store
        .pages(a, n, "absent%", false, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    for text in ["Русский", "中文", "_literal"] {
        assert_eq!(
            store.pages(a, n, text, false, 0).await.unwrap().items.len(),
            1
        );
    }
    let draft = Draft {
        revision: None,
        id,
        page: Some(id),
        base_revision: Some(page.revision),
        name: page.name.clone(),
        markdown: "Unpublished private secret".into(),
    };
    store.save_draft(c, n, draft.clone()).await.unwrap();
    let mut conflicting = draft.clone();
    conflicting.markdown = "Concurrent draft".into();
    assert!(matches!(
        store.save_draft(c, n, conflicting).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        store.discard_draft(c, n, id, None).await,
        Err(Error::Conflict)
    ));
    assert!(store.draft(a, n, id).await.unwrap().is_none());
    assert!(store
        .pages(a, n, "Unpublished", false, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    let rename = Write::new(
        WriteInput {
            operation: Uuid::new_v4(),
            page: id,
            expected_revision: Some(page.revision),
            change: ChangeInput::Rename {
                name: "Renamed".into(),
            },
        },
        &limits,
    )
    .unwrap();
    let renamed = store.write(c, n, rename).await.unwrap();
    assert_eq!(store.resolve(a, n, "Exact 页面 ").await.unwrap().id, id);
    assert_eq!(store.resolve(a, n, "Renamed").await.unwrap().id, id);
    assert!(matches!(
        store
            .write(
                a,
                n,
                Write::new(
                    WriteInput {
                        operation: Uuid::new_v4(),
                        page: id,
                        expected_revision: Some(page.revision),
                        change: ChangeInput::Save {
                            name: page.name.clone(),
                            markdown: "Conflicting write".into()
                        }
                    },
                    &limits
                )
                .unwrap()
            )
            .await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        store.draft(c, n, id).await.unwrap().unwrap().markdown,
        draft.markdown
    );
    let second = Uuid::new_v4();
    assert!(matches!(
        store
            .write(
                a,
                n,
                Write::new(
                    WriteInput {
                        operation: Uuid::new_v4(),
                        page: second,
                        expected_revision: None,
                        change: ChangeInput::Save {
                            name: page.name.clone(),
                            markdown: "collision".into()
                        }
                    },
                    &limits
                )
                .unwrap()
            )
            .await,
        Err(Error::NameTaken)
    ));
    let sub = Subscription::new(
        SubscriptionId::new(),
        actors[0].1.id(),
        url::Url::parse("https://wiki.example/feed").unwrap(),
        "Private subscription".into(),
    );
    repository
        .save_subscription(None, sub.clone())
        .await
        .unwrap();
    let subscription = sub.id().as_uuid();
    store.bind(a, subscription, Some((n, id))).await.unwrap();
    assert!(matches!(
        store.binding(b, subscription).await,
        Err(Error::NotFound)
    ));
    assert_eq!(
        store
            .binding(a, subscription)
            .await
            .unwrap()
            .page
            .unwrap()
            .name,
        "Renamed"
    );
    let trashed = store
        .write(
            a,
            n,
            Write::new(
                WriteInput {
                    operation: Uuid::new_v4(),
                    page: id,
                    expected_revision: Some(renamed.revision),
                    change: ChangeInput::Trash,
                },
                &limits,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(store.page(b, n, id).await, Err(Error::NotFound)));
    assert!(matches!(
        store.history(b, n, id, 0).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.resolve(b, n, "Renamed").await,
        Err(Error::NotFound)
    ));
    assert!(store
        .pages(a, n, "", false, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(matches!(
        store.revision(b, n, id, page.revision).await,
        Err(Error::NotFound)
    ));
    assert!(store.binding(a, subscription).await.unwrap().linked);
    assert!(store.binding(a, subscription).await.unwrap().page.is_none());
    let restored = store
        .write(
            a,
            n,
            Write::new(
                WriteInput {
                    operation: Uuid::new_v4(),
                    page: id,
                    expected_revision: Some(trashed.revision),
                    change: ChangeInput::Restore,
                },
                &limits,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(store.binding(a, subscription).await.unwrap().page.is_some());
    let previous = store
        .write(
            a,
            n,
            Write::new(
                WriteInput {
                    operation: Uuid::new_v4(),
                    page: id,
                    expected_revision: Some(restored.revision),
                    change: ChangeInput::RestoreRevision {
                        revision: page.revision,
                    },
                },
                &limits,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(previous.markdown, page.markdown);
    assert_ne!(previous.revision, page.revision);
    assert!(store.history(a, n, id, 0).await.unwrap().has_more);
    assert_eq!(store.history(a, n, id, 2).await.unwrap().items.len(), 2);
    // A compound FK rejects a foreign-namespace page even for direct SQL writes.
    assert!(sqlx::query(
        "INSERT INTO wiki_page_names(namespace,name,page) VALUES($1,'foreign',$2)"
    )
    .bind(other)
    .bind(id)
    .execute(pool)
    .await
    .is_err());
    // Hold the namespace lock, revoke membership, then release a pending save.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM wiki_namespaces WHERE id=$1 FOR UPDATE")
        .bind(n)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM wiki_members WHERE namespace=$1 AND account=$2")
        .bind(n)
        .bind(c.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let pending = Write::new(
        WriteInput {
            operation: Uuid::new_v4(),
            page: id,
            expected_revision: Some(previous.revision),
            change: ChangeInput::Save {
                name: previous.name.clone(),
                markdown: "Unauthorized".into(),
            },
        },
        &limits,
    )
    .unwrap();
    let cloned = store.clone();
    let task = tokio::spawn(async move { cloned.write(c, n, pending).await });
    tx.commit().await.unwrap();
    assert!(matches!(task.await.unwrap(), Err(Error::NotFound)));
    assert_eq!(store.page(a, n, id).await.unwrap().markdown, page.markdown);
    assert!(matches!(store.draft(c, n, id).await, Err(Error::NotFound)));
    sqlx::query("DELETE FROM ingest_jobs WHERE origin_key='https://wiki.example'")
        .execute(pool)
        .await
        .unwrap();
    let removable = Subscription::new(
        SubscriptionId::new(),
        actors[0].1.id(),
        url::Url::parse("https://wiki.example/removable").unwrap(),
        "Disposable binding".into(),
    );
    repository
        .save_subscription(None, removable.clone())
        .await
        .unwrap();
    store
        .bind(a, removable.id().as_uuid(), Some((n, id)))
        .await
        .unwrap();
    repository.delete_subscription(&removable).await.unwrap();
    assert_eq!(store.page(a, n, id).await.unwrap().markdown, page.markdown);
    assert!(matches!(
        store.binding(a, removable.id().as_uuid()).await,
        Err(Error::NotFound)
    ));
    // Measure the production search query, without disabling sequential scans.
    let mut plan_tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO wiki_pages(namespace,id,revision,name,markdown,deleted,author,updated_at) SELECT $1,gen_random_uuid(),gen_random_uuid(),'Planner '||i,CASE WHEN i=1500 THEN 'needleuniquemarker' ELSE repeat('ordinary wiki paragraph ',50) END,false,$2,clock_timestamp() FROM generate_series(1,5000) i").bind(n).bind(a.to_string()).execute(&mut *plan_tx).await.unwrap();
    sqlx::query("ANALYZE wiki_pages")
        .execute(&mut *plan_tx)
        .await
        .unwrap();
    let explain = format!(
        "EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) {}",
        include_str!("../../src/wiki/pages.sql")
    );
    let plan: serde_json::Value = sqlx::query_scalar(&explain)
        .bind(n)
        .bind(false)
        .bind("needleuniquemarker")
        .bind("%needleuniquemarker%")
        .bind(3i64)
        .bind(0i64)
        .bind(200i32)
        .fetch_one(&mut *plan_tx)
        .await
        .unwrap();
    assert!(
        plan.to_string().contains("wiki_pages_body_search"),
        "selective literal search must use the GIN index: {plan}"
    );
    eprintln!(
        "wiki_search_benchmark pages=5000 execution_ms={}",
        plan[0]["Execution Time"]
    );
    plan_tx.rollback().await.unwrap();
    // Keep nonempty page/history/draft/link/binding fixtures for backup/restore.
}
