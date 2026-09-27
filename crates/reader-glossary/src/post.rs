use crate::{
    definitions, styled_html, Definition, GlossaryError, MarkKind, StyledText, TextMark,
    CHANNEL_USERNAME,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PostProjection {
    Indexed {
        content: StyledText,
        definitions: Vec<Definition>,
    },
    Unindexed {
        error: String,
    },
}

/// One source post identity. Raw archive lines are retained byte-for-byte;
/// Telegram post JSON is a projection of the separately retained raw response.
/// Missing dates remain missing. Unsupported formatting never drops the post.
pub struct ObservedPost {
    id: i64,
    published_at: Option<DateTime<Utc>>,
    edited_at: Option<i64>,
    raw: String,
    projection: PostProjection,
}
impl ObservedPost {
    pub fn archive(raw: String) -> Result<Self, GlossaryError> {
        let value: Value = serde_json::from_str(&raw).map_err(|_| GlossaryError::Protocol)?;
        if value["channel"].as_str() != Some(CHANNEL_USERNAME) {
            return Err(GlossaryError::Identity);
        }
        let id = value["id"]
            .as_i64()
            .filter(|v| *v > 0)
            .ok_or(GlossaryError::Identity)?;
        if value["permalink"].as_str()
            != Some(format!("https://t.me/{CHANNEL_USERNAME}/{id}").as_str())
        {
            return Err(GlossaryError::Identity);
        }
        let published_at = match &value["timestamp"] {
            Value::Null => None,
            Value::String(s) => Some(
                DateTime::parse_from_rfc3339(s)
                    .map_err(|_| GlossaryError::Protocol)?
                    .with_timezone(&Utc),
            ),
            _ => return Err(GlossaryError::Protocol),
        };
        let content = value["formatted_html"]
            .as_str()
            .ok_or(GlossaryError::Protocol)?;
        let projection = project(styled_html(content));
        Ok(Self {
            id,
            published_at,
            edited_at: None,
            raw,
            projection,
        })
    }
    pub fn telegram(value: &Value, channel_id: i64) -> Result<Self, GlossaryError> {
        if channel_id >= 0
            || value["chat"]["id"].as_i64() != Some(channel_id)
            || value["chat"]["type"].as_str() != Some("channel")
        {
            return Err(GlossaryError::Identity);
        }
        let id = value["message_id"]
            .as_i64()
            .filter(|v| *v > 0)
            .ok_or(GlossaryError::Identity)?;
        let date = value["date"].as_i64().ok_or(GlossaryError::Protocol)?;
        let published_at = Some(DateTime::from_timestamp(date, 0).ok_or(GlossaryError::Protocol)?);
        let edited_at = value
            .get("edit_date")
            .map(|v| {
                v.as_i64()
                    .filter(|edit| *edit >= date)
                    .ok_or(GlossaryError::Protocol)
            })
            .transpose()?;
        let projection = project(telegram_text(value));
        Ok(Self {
            id,
            published_at,
            edited_at,
            raw: serde_json::to_string(value).map_err(|_| GlossaryError::Protocol)?,
            projection,
        })
    }
    pub fn id(&self) -> i64 {
        self.id
    }
    pub fn published_at(&self) -> Option<DateTime<Utc>> {
        self.published_at
    }
    pub fn edited_at(&self) -> Option<i64> {
        self.edited_at
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn projection(&self) -> &PostProjection {
        &self.projection
    }
}

fn project(content: Result<StyledText, GlossaryError>) -> PostProjection {
    match content
        .and_then(|content| definitions(&content).map(|definitions| (content, definitions)))
    {
        Ok((content, definitions)) => PostProjection::Indexed {
            content,
            definitions,
        },
        Err(error) => PostProjection::Unindexed {
            error: error.to_string(),
        },
    }
}

pub fn telegram_text(value: &Value) -> Result<StyledText, GlossaryError> {
    if value.get("rich_message").is_some() {
        // Raw update is still durable; never acknowledge unsupported formatting as indexed.
        return Err(GlossaryError::Unsupported);
    }
    let (text, entities) = if let Some(text) = value.get("text") {
        (
            text.as_str().ok_or(GlossaryError::Protocol)?,
            value.get("entities"),
        )
    } else if let Some(text) = value.get("caption") {
        (
            text.as_str().ok_or(GlossaryError::Protocol)?,
            value.get("caption_entities"),
        )
    } else {
        ("", None)
    };
    let mut marks = Vec::new();
    if let Some(entities) = entities {
        for entity in entities.as_array().ok_or(GlossaryError::Protocol)? {
            let start = entity["offset"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or(GlossaryError::Formatting)?;
            let length = entity["length"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or(GlossaryError::Formatting)?;
            let end = start
                .checked_add(length)
                .filter(|v| *v > start)
                .ok_or(GlossaryError::Formatting)?;
            let a = crate::utf16_byte(text, start)?;
            let b = crate::utf16_byte(text, end)?;
            let style = match entity["type"].as_str().ok_or(GlossaryError::Formatting)? {
                "bold" => MarkKind::Bold,
                "italic" => MarkKind::Italic,
                "underline" => MarkKind::Underline,
                "strikethrough" => MarkKind::Strike,
                "spoiler" => MarkKind::Spoiler,
                "code" => MarkKind::Code,
                "pre" => MarkKind::Pre,
                "blockquote" | "expandable_blockquote" => MarkKind::Quote,
                "text_link" => MarkKind::Link {
                    url: entity["url"]
                        .as_str()
                        .ok_or(GlossaryError::Formatting)?
                        .into(),
                },
                "url" => MarkKind::Link {
                    url: text[a..b].into(),
                },
                "mention" | "hashtag" | "cashtag" | "bot_command" | "email" | "phone_number" => {
                    continue
                }
                _ => return Err(GlossaryError::Unsupported),
            };
            marks.push(TextMark { start, end, style });
        }
    }
    StyledText::new(text.into(), marks)
}
