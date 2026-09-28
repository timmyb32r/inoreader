use super::*;
use crate::{BrowserHttpResponse, BuiltInAdapter, SourceKind};
use reader_core::SourceId;
use std::sync::Mutex;
#[test]
fn cookie_validation_preserves_values_and_scopes() {
    let cookies = Cookies::parse("z_c0=Exact%2BValue==; _xsrf=AbC").unwrap();
    let header = cookies.header("/api/v4/me").unwrap();
    assert_eq!(header.to_str().unwrap(), "z_c0=Exact%2BValue==; _xsrf=AbC");
    assert!(header.is_sensitive());
    let cookies = Cookies::parse("z_c0=test; analytics=123,456; quoted=\"Exact==\"").unwrap();
    assert_eq!(
        cookies.header("/api/v4/me").unwrap(),
        "z_c0=test; analytics=123,456; quoted=\"Exact==\""
    );
    for raw in ["z_c0=test; bad=\"unclosed", "z_c0=test; bad=a\"b"] {
        assert!(Cookies::parse(raw).is_err());
    }
    let raw="z_c0\tExact==\t.zhihu.com\t/\t1\t2\t3\t4\t5\t6\t7\t8\nother\tScoped\twww.zhihu.com\t/private\t1\t2\t3\t4\t5\t6\t7\t8";
    let c = Cookies::parse(raw).unwrap();
    assert_eq!(c.header("/api/v4/me").unwrap(), "z_c0=Exact==");
    assert_eq!(
        c.header("/private/x").unwrap(),
        "z_c0=Exact==; other=Scoped"
    );
    assert_eq!(c.header("/privatex").unwrap(), "z_c0=Exact==");
    for raw in [
        "",
        "_xsrf=x",
        "z_c0=x\r\nInjected:y",
        "z_c0=x; z_c0=y",
        "z_c0=x; bad name=x",
        "z_c0=x y",
        "z_c0=x\\y",
        "z_c0=",
    ] {
        assert!(Cookies::parse(raw).is_err(), "accepted invalid cookies");
    }
    assert!(Cookies::parse(&raw.replace(".zhihu.com", ".evil.test")).is_err());
}
#[test]
fn source_constructor_and_serde_reject_wrong_origins_and_paths() {
    for raw in [
        "http://www.zhihu.com/people/a/posts",
        "https://evil.test/people/a/posts",
        "https://www.zhihu.com/people/a/posts?x=1",
        "https://user@www.zhihu.com/people/a/posts",
        "https://www.zhihu.com/people/a/b/posts",
    ] {
        let kind = SourceKind::BuiltIn(BuiltInAdapter::Zhihu {
            max_pages: NonZeroUsize::new(1).unwrap(),
        });
        assert!(
            SourceDefinition::new(SourceId::new(), Url::parse(raw).unwrap(), kind.clone()).is_err()
        );
        assert!(serde_json::from_value::<SourceDefinition>(
            serde_json::json!({"id":Uuid::new_v4(),"url":raw,"kind":kind,"revision":0})
        )
        .is_err());
    }
}
struct Memory {
    encrypted: Mutex<Option<Vec<u8>>>,
    owner: Uuid,
}
#[async_trait]
impl Store for Memory {
    async fn load(&self, owner: Uuid) -> Result<Option<Vec<u8>>, Error> {
        if owner != self.owner {
            return Ok(None);
        }
        Ok(self.encrypted.lock().unwrap().clone())
    }
    async fn save(&self, owner: Uuid, encrypted: Vec<u8>, _: NonZeroUsize) -> Result<(), Error> {
        assert_eq!(owner, self.owner);
        *self.encrypted.lock().unwrap() = Some(encrypted);
        Ok(())
    }
    async fn remove(&self, owner: Uuid) -> Result<(), Error> {
        assert_eq!(owner, self.owner);
        *self.encrypted.lock().unwrap() = None;
        Ok(())
    }
    async fn source_owner(&self, _: &SourceDefinition) -> Result<Uuid, Error> {
        Ok(self.owner)
    }
}
struct TestVault;
impl Vault for TestVault {
    fn seal(&self, _: Uuid, s: &str) -> Result<Vec<u8>, Error> {
        Ok(s.as_bytes().to_vec())
    }
    fn open(&self, _: Uuid, s: &[u8]) -> Result<String, Error> {
        String::from_utf8(s.to_vec()).map_err(|_| Error::Storage)
    }
}
struct Http {
    status: u16,
    body: serde_json::Value,
    paths: Mutex<Vec<String>>,
}
#[async_trait]
impl BrowserHttpClient for Http {
    async fn execute(&self, r: PreparedRequest) -> Result<BrowserHttpResponse, FetchError> {
        assert_eq!(r.url.host_str(), Some("www.zhihu.com"));
        assert_eq!(r.url.scheme(), "https");
        assert!(r.headers[http::header::COOKIE].is_sensitive());
        assert_eq!(r.headers[http::header::COOKIE], "z_c0=test");
        self.paths.lock().unwrap().push(r.url.path().into());
        Ok(BrowserHttpResponse {
            final_url: r.url,
            status: self.status,
            headers: HeaderMap::new(),
            body: serde_json::to_vec(&self.body).unwrap(),
        })
    }
}
fn service(status: u16, body: serde_json::Value) -> (Service, Arc<Memory>) {
    let store = Arc::new(Memory {
        encrypted: Mutex::new(Some(b"z_c0=test".to_vec())),
        owner: Uuid::new_v4(),
    });
    (
        Service::new(
            store.clone(),
            Arc::new(TestVault),
            Arc::new(Http {
                status,
                body,
                paths: Mutex::new(vec![]),
            }),
            NonZeroUsize::new(1).unwrap(),
        ),
        store,
    )
}
#[tokio::test]
async fn rejected_session_never_replaces_saved_secret() {
    let (svc, store) = service(403, serde_json::json!({}));
    assert!(matches!(
        svc.save(store.owner, "z_c0=test".into()).await,
        Err(Error::Session)
    ));
    assert_eq!(
        store.load(store.owner).await.unwrap().unwrap(),
        b"z_c0=test"
    );
    assert!(!svc.configured(Uuid::new_v4()).await.unwrap());
    svc.remove(store.owner).await.unwrap();
    assert!(!svc.configured(store.owner).await.unwrap());
}
#[tokio::test]
async fn public_listing_preserves_identity_date_and_rejects_gated_content() {
    let row = serde_json::json!({"id":"001970589926022412116","title":"原始标题","excerpt":"<p>Exact</p>","created":1700000000,"content":"<p>Exact public body</p>","content_need_truncated":false,"force_login_when_click_read_more":false,"paid_info":{}});
    let (svc, _) = service(
        200,
        serde_json::json!({"data":[row.clone()],"paging":{"is_end":true,"next":"https://evil.test"}}),
    );
    let source = SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://www.zhihu.com/people/ververica/posts").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Zhihu {
            max_pages: NonZeroUsize::new(1).unwrap(),
        }),
    )
    .unwrap();
    let records = svc
        .collect(&source, NonZeroUsize::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].upstream_id(), "001970589926022412116");
    let serialized = serde_json::to_string(&records).unwrap();
    assert!(serialized.contains("原始标题"));
    assert!(serialized.contains("Exact public body"));
    for (field, value) in [
        ("paid_info", serde_json::json!({"price":1})),
        ("content_need_truncated", serde_json::json!(true)),
        ("force_login_when_click_read_more", serde_json::json!(true)),
        ("id", serde_json::json!(123)),
    ] {
        let mut gated = row.clone();
        gated[field] = value;
        let (svc, _) = service(
            200,
            serde_json::json!({"data":[gated],"paging":{"is_end":true}}),
        );
        assert!(svc
            .collect(&source, NonZeroUsize::new(1).unwrap())
            .await
            .is_err());
    }
    assert!(serialized.contains("2023-11-14T22:13:20+00:00"));
    let (svc, _) = service(
        200,
        serde_json::json!({"data":[row.clone(),row],"paging":{"is_end":true}}),
    );
    assert!(svc
        .collect(&source, NonZeroUsize::new(1).unwrap())
        .await
        .is_err());
}

#[test]
fn real_subscription_profile_urls_and_posts_urls_keep_their_exact_spelling() {
    for (raw, slug) in [
        ("https://www.zhihu.com/people/ververica", "ververica"),
        ("https://www.zhihu.com/org/datafuntalk", "datafuntalk"),
        ("https://www.zhihu.com/people/ververica/posts", "ververica"),
        ("https://www.zhihu.com/org/datafuntalk/posts", "datafuntalk"),
    ] {
        let source = SourceDefinition::new(
            SourceId::new(),
            Url::parse(raw).unwrap(),
            SourceKind::BuiltIn(BuiltInAdapter::Zhihu {
                max_pages: NonZeroUsize::new(1).unwrap(),
            }),
        )
        .unwrap();
        assert_eq!(author(source.url()), Some(slug));
        assert_eq!(source.url().as_str(), raw);
        let decoded: SourceDefinition =
            serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
        assert_eq!(decoded, source);
    }
}
