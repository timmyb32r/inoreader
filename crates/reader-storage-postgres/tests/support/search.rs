use super::*;
use reader_application::{SearchKind, SearchLimits, SearchLimitsInput, SearchPort, SearchRequest};
use reader_wiki::{ChangeInput, LimitsInput, Role, Store, Write, WriteInput};
pub async fn verify(pool: &PgPool) {
    let repo =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let mut actors = vec![];
    for i in 0..3 {
        let account = AccountRecord {
            id: AccountId::new(),
            username: format!("search-{}", Uuid::new_v4()),
            password_hash: "fixture".into(),
            admin: i == 2,
            auth_revision: 0,
            revision: 0,
        };
        let ws = Workspace::new(WorkspaceId::new(), account.id, "Search fixture".into());
        repo.create_account_and_workspace(account.clone(), ws.clone())
            .await
            .unwrap();
        actors.push((account, ws));
    }
    let owner = actors[0].0.id.as_uuid();
    let stranger = actors[1].0.id.as_uuid();
    let admin = actors[2].0.id.as_uuid();
    let limits = SearchLimits::new(SearchLimitsInput {
        query_bytes: 512,
        page_size: 1,
        excerpt_characters: 80,
    })
    .unwrap();
    let search = reader_storage_postgres::PostgresSearchStore::new(pool.clone(), limits.clone());
    let query = |text: &str, kind, ns, offset| {
        SearchRequest::new(&limits, text.into(), kind, ns, None, offset).unwrap()
    };
    for (_, ws) in &actors {
        let article = Article {
            id: ArticleId::new(),
            key: DedupKey {
                location: ArticleLocation::from(
                    url::Url::parse("https://search.example/item").unwrap(),
                ),
                title: "SearchProbe 中文 100% literal".into(),
                description: Some("Unique_body_token".into()),
            },
            state: ArticleState::default(),
            first_arrived_at: Utc::now(),
            origins: vec![],
            revision: 0,
        };
        sqlx::query("INSERT INTO articles(id,revision,document) VALUES($1,0,$2)")
            .bind(format!("{}/{}", ws.id().as_uuid(), article.id.as_uuid()))
            .bind(serde_json::to_string(&article).unwrap())
            .execute(pool)
            .await
            .unwrap();
    }
    let found = search
        .search(owner, query("SearchProbe", SearchKind::News, None, 0))
        .await
        .unwrap();
    assert_eq!(found.items.len(), 1);
    assert!(!found.has_more);
    assert_eq!(
        search
            .search(owner, query("中文", SearchKind::All, None, 0))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        search
            .search(owner, query("100%", SearchKind::All, None, 0))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(search
        .search(owner, query("100_", SearchKind::All, None, 0))
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        search
            .search(owner, query("body_token", SearchKind::All, None, 0))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    search_content::verify(pool, owner, actors[0].1.id(), &limits).await;
    let wiki_limits: reader_wiki::Limits = LimitsInput {
        name_bytes: 512,
        markdown_bytes: 524288,
        search_excerpt_characters: 200,
        search_bytes: 512,
        page_size: 20,
        draft_save_delay_ms: 1000,
    }
    .try_into()
    .unwrap();
    let wiki = reader_storage_postgres::PostgresWikiStore::new(pool.clone(), wiki_limits.clone());
    let ns = Uuid::new_v4();
    wiki.create_namespace(owner, ns, "Private Search")
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let page = wiki
        .write(
            owner,
            ns,
            Write::new(
                WriteInput {
                    operation: Uuid::new_v4(),
                    page: id,
                    expected_revision: None,
                    change: ChangeInput::Save {
                        name: "SearchProbe".into(),
                        markdown: "private secret 中文".into(),
                    },
                },
                &wiki_limits,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    for actor in [stranger, admin] {
        assert!(search
            .search(actor, query("private secret", SearchKind::Wiki, None, 0))
            .await
            .unwrap()
            .items
            .is_empty());
        assert!(matches!(
            search
                .search(actor, query("SearchProbe", SearchKind::Wiki, Some(ns), 0))
                .await,
            Err(RepositoryError::NotFound)
        ));
    }
    wiki.set_member(owner, ns, &actors[1].0.username, Some(Role::Reader))
        .await
        .unwrap();
    assert_eq!(
        search
            .search(stranger, query("private secret", SearchKind::Wiki, None, 0))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    wiki.set_member(owner, ns, &actors[1].0.username, None)
        .await
        .unwrap();
    assert!(search
        .search(stranger, query("private secret", SearchKind::Wiki, None, 0))
        .await
        .unwrap()
        .items
        .is_empty());
    let first = search
        .search(owner, query("SearchProbe", SearchKind::All, None, 0))
        .await
        .unwrap();
    assert_eq!(first.items[0].title, "SearchProbe");
    assert!(first.has_more);
    assert!(
        !search
            .search(owner, query("SearchProbe", SearchKind::All, None, 1))
            .await
            .unwrap()
            .has_more
    );
    wiki.write(
        owner,
        ns,
        Write::new(
            WriteInput {
                operation: Uuid::new_v4(),
                page: id,
                expected_revision: Some(page.revision),
                change: ChangeInput::Trash,
            },
            &wiki_limits,
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(search
        .search(owner, query("private secret", SearchKind::Wiki, None, 0))
        .await
        .unwrap()
        .items
        .is_empty());
}
