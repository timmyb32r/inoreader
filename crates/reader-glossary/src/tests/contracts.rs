use crate::*;
use serde_json::json;

fn config() -> GlossaryConfig {
    GlossaryConfig {
        polling_timeout_seconds: 25,
        request_timeout_seconds: 45,
        connect_timeout_seconds: 10,
        lease_seconds: 120,
        batch_size: 100,
        max_response_bytes: 100000,
        max_archive_line_bytes: 100000,
        retry_initial_seconds: 5,
        retry_max_seconds: 300,
        public_page_interval_milliseconds: 2000,
        history_reconcile_interval_seconds: 21600,
        history_pages_per_run: 5,
        worker_poll_milliseconds: 1000,
        workers: 2,
    }
}
#[test]
fn policy_and_credentials_reject_invalid_execution_state() {
    GlossaryPolicy::new(config()).unwrap();
    for change in [
        |c: &mut GlossaryConfig| c.lease_seconds = 90,
        |c: &mut GlossaryConfig| c.batch_size = 101,
        |c: &mut GlossaryConfig| c.max_response_bytes = 0,
        |c: &mut GlossaryConfig| c.workers = 0,
    ] {
        let mut c = config();
        change(&mut c);
        assert!(GlossaryPolicy::new(c).is_err());
    }
    let valid = BotToken::new("123:private-secret".into()).unwrap();
    assert!(!format!("{valid:?}").contains("private-secret"));
    for value in ["0:key", "123:a/b", "123:a?b", "123:", "123:key\n"] {
        assert!(BotToken::new(value.into()).is_err());
    }
    assert!(serde_json::from_value::<ChannelBinding>(
        json!({"bot_id":1,"bot_username":"bot","channel_id":1})
    )
    .is_err());
}
#[test]
fn batches_keep_exact_envelope_and_reject_ambiguous_order() {
    let raw=" {\"ok\":true,\"future\":{\"retain\":1},\"result\":[{\"update_id\":7,\"unknown\":true},{\"update_id\":12}]} ";
    let batch = UpdateBatch::new(raw.into()).unwrap();
    assert_eq!(batch.raw(), raw);
    assert_eq!(batch.next_offset(), Some(13));
    for ids in [vec![2, 2], vec![3, 2], vec![-1], vec![i64::MAX]] {
        let raw=json!({"ok":true,"result":ids.into_iter().map(|id|json!({"update_id":id})).collect::<Vec<_>>()}).to_string();
        assert!(UpdateBatch::new(raw).is_err());
    }
}
#[test]
fn public_history_preserves_raw_and_never_interprets_missing_ids_as_deletes() {
    let raw = r#"<div class="tgme_widget_message" data-post="reading_data_news/9"><div class="tgme_widget_message_text"><b>CDC</b> — текст</div><time datetime="2026-09-27T10:00:00+00:00"></time></div><div class="tgme_widget_message" data-post="reading_data_news/3"><div class="tgme_widget_message_text"><b>SQL</b> — запросы</div></div>"#;
    let page = parse_public_page(raw.into(), None).unwrap();
    assert_eq!(page.raw, raw);
    assert_eq!(page.posts.len(), 2);
    assert_eq!(page.before, Some(3));
    assert!(parse_public_page("<html>blocked</html>".into(), None).is_err());
    assert!(parse_public_page(
        raw.replace("reading_data_news/3", "another_channel/3"),
        None
    )
    .is_err());
}
