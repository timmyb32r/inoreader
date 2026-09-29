use super::*;
use reader_storage_postgres::PostgresWikiStore;
use reader_wiki::{
    ChangeInput, Collection, Error, Limits, LimitsInput, Role, Store, Write, WriteInput,
};

pub async fn verify(pool: &PgPool) {
    let repository =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let mut actors = Vec::new();
    for _ in 0..2 {
        let account = AccountRecord {
            id: AccountId::new(),
            username: format!("organization-{}", Uuid::new_v4()),
            password_hash: "test".into(),
            admin: false,
            auth_revision: 0,
            revision: 0,
        };
        let workspace = Workspace::new(WorkspaceId::new(), account.id, "Wiki".into());
        repository
            .create_account_and_workspace(account.clone(), workspace)
            .await
            .unwrap();
        actors.push(account);
    }
    let a = actors[0].id.as_uuid();
    let b = actors[1].id.as_uuid();
    let limits: Limits = LimitsInput {
        name_bytes: 512,
        markdown_bytes: 524288,
        search_bytes: 512,
        search_excerpt_characters: 200,
        page_size: 2,
        draft_save_delay_ms: 1000,
    }
    .try_into()
    .unwrap();
    let store = PostgresWikiStore::new(pool.clone(), limits.clone());
    let n = Uuid::new_v4();
    let other = Uuid::new_v4();
    store.create_namespace(a, n, "Organization").await.unwrap();
    store.create_namespace(b, other, "Other").await.unwrap();
    let root = store.subscription_root(a, n).await.unwrap();
    assert_eq!(root.id, store.subscription_root(a, n).await.unwrap().id);
    let command = |page, revision, change| {
        Write::new(
            WriteInput {
                operation: Uuid::new_v4(),
                page,
                expected_revision: revision,
                change,
            },
            &limits,
        )
        .unwrap()
    };
    let p = store
        .write(
            a,
            n,
            command(
                Uuid::new_v4(),
                None,
                ChangeInput::Save {
                    name: "Child".into(),
                    markdown: "Keep 中文\nexact".into(),
                },
            ),
        )
        .await
        .unwrap();
    let detached = p.revision;
    let p = store
        .write(
            a,
            n,
            command(
                p.id,
                Some(p.revision),
                ChangeInput::SetParent {
                    parent: Some(root.id),
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!(p.parent, Some(root.id));
    assert_eq!(p.markdown, "Keep 中文\nexact");
    assert!(store
        .collection(a, n, Collection::Standalone, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    let org = store.organization(a, n, root.id, 0).await.unwrap();
    assert_eq!(org.children.items[0].id, p.id);
    assert!(matches!(
        store
            .write(
                a,
                n,
                command(
                    root.id,
                    Some(root.revision),
                    ChangeInput::SetParent { parent: Some(p.id) }
                )
            )
            .await,
        Err(Error::Invalid(_))
    ));
    let foreign = store.subscription_root(b, other).await.unwrap();
    assert!(matches!(
        store
            .write(
                a,
                n,
                command(
                    p.id,
                    Some(p.revision),
                    ChangeInput::SetParent {
                        parent: Some(foreign.id)
                    }
                )
            )
            .await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.favorite(b, n, p.id, true).await,
        Err(Error::NotFound)
    ));
    store
        .set_member(a, n, &actors[1].username, Some(Role::Reader))
        .await
        .unwrap();
    store.favorite(b, n, p.id, true).await.unwrap();
    store.favorite(b, n, p.id, true).await.unwrap();
    assert_eq!(
        store
            .collection(b, n, Collection::Favorites, 0)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(store
        .collection(a, n, Collection::Favorites, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(matches!(
        store
            .write(
                b,
                n,
                command(
                    p.id,
                    Some(p.revision),
                    ChangeInput::SetParent { parent: None }
                )
            )
            .await,
        Err(Error::Forbidden)
    ));
    store
        .set_member(a, n, &actors[1].username, None)
        .await
        .unwrap();
    assert!(matches!(
        store.collection(b, n, Collection::Favorites, 0).await,
        Err(Error::NotFound)
    ));
    let restored = store
        .write(
            a,
            n,
            command(
                p.id,
                Some(p.revision),
                ChangeInput::RestoreRevision { revision: detached },
            ),
        )
        .await
        .unwrap();
    assert_eq!(restored.parent, None);
    assert_eq!(
        store
            .collection(a, n, Collection::Standalone, 0)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    // Competing A→B and B→A writes serialize on the namespace lock.
    let first = store.write(
        a,
        n,
        command(
            p.id,
            Some(restored.revision),
            ChangeInput::SetParent {
                parent: Some(root.id),
            },
        ),
    );
    let second = store.write(
        a,
        n,
        command(
            root.id,
            Some(root.revision),
            ChangeInput::SetParent { parent: Some(p.id) },
        ),
    );
    let (x, y) = tokio::join!(first, second);
    assert_ne!(x.is_ok(), y.is_ok());
    store.favorite(a, n, p.id, true).await.unwrap();
    store.favorite(a, n, p.id, false).await.unwrap();
    assert!(!store.organization(a, n, p.id, 0).await.unwrap().favorite);
    // Paginated children and collections must not silently lose the tail.
    for i in 0..3 {
        let p = store
            .write(
                a,
                n,
                command(
                    Uuid::new_v4(),
                    None,
                    ChangeInput::Save {
                        name: format!("Extra {i}"),
                        markdown: String::new(),
                    },
                ),
            )
            .await
            .unwrap();
        store.favorite(a, n, p.id, true).await.unwrap();
    }
    let list = store
        .collection(a, n, Collection::Favorites, 0)
        .await
        .unwrap();
    assert_eq!(list.items.len(), 2);
    assert!(list.has_more);
    assert_eq!(
        store
            .collection(a, n, Collection::Favorites, 2)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}
