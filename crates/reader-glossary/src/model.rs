use crate::GlossaryError;
use serde::{Deserialize, Serialize};

pub const PARSER_VERSION: &str = "channel_definitions_v1";
pub const CHANNEL_USERNAME: &str = "reading_data_news";

/// Formatting intervals are UTF-16 code units, end-exclusive, like Telegram and
/// JavaScript. Every interval must lie on a Unicode scalar boundary in `text`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Pre,
    Spoiler,
    Quote,
    Link { url: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextMark {
    pub start: usize,
    pub end: usize,
    pub style: MarkKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TextWire")]
pub struct StyledText {
    text: String,
    marks: Vec<TextMark>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextWire {
    text: String,
    marks: Vec<TextMark>,
}

impl TryFrom<TextWire> for StyledText {
    type Error = GlossaryError;
    fn try_from(v: TextWire) -> Result<Self, Self::Error> {
        Self::new(v.text, v.marks)
    }
}

impl StyledText {
    pub fn new(text: String, marks: Vec<TextMark>) -> Result<Self, GlossaryError> {
        for mark in &marks {
            if mark.start >= mark.end {
                return Err(GlossaryError::Formatting);
            }
            utf16_byte(&text, mark.start)?;
            utf16_byte(&text, mark.end)?;
            if let MarkKind::Link { url } = &mark.style {
                if url.is_empty() {
                    return Err(GlossaryError::Formatting);
                }
            }
        }
        Ok(Self { text, marks })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn marks(&self) -> &[TextMark] {
        &self.marks
    }
    pub fn slice(&self, start: usize, end: usize) -> Result<Self, GlossaryError> {
        let a = utf16_byte(&self.text, start)?;
        let b = utf16_byte(&self.text, end)?;
        let text = self.text.get(a..b).ok_or(GlossaryError::Formatting)?;
        let marks = self
            .marks
            .iter()
            .filter_map(|m| {
                let left = m.start.max(start);
                let right = m.end.min(end);
                (left < right).then(|| TextMark {
                    start: left - start,
                    end: right - start,
                    style: m.style.clone(),
                })
            })
            .collect();
        Self::new(text.to_owned(), marks)
    }
}

pub fn utf16_byte(text: &str, offset: usize) -> Result<usize, GlossaryError> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return Err(GlossaryError::Formatting);
        }
    }
    if units == offset {
        Ok(text.len())
    } else {
        Err(GlossaryError::Formatting)
    }
}

/// A definition is a verified exact leading name plus its complete paragraph.
/// Its position is in the post's UTF-16 projection, never an HTML byte offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DefinitionWire")]
pub struct Definition {
    term: String,
    position: usize,
    paragraph: StyledText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionWire {
    term: String,
    position: usize,
    paragraph: StyledText,
}
impl TryFrom<DefinitionWire> for Definition {
    type Error = GlossaryError;
    fn try_from(v: DefinitionWire) -> Result<Self, Self::Error> {
        let definition =
            crate::parse_paragraph(v.paragraph, v.position)?.ok_or(GlossaryError::Formatting)?;
        if definition.term != v.term {
            return Err(GlossaryError::Formatting);
        }
        Ok(definition)
    }
}
impl Definition {
    pub(crate) fn parsed(term: String, position: usize, paragraph: StyledText) -> Self {
        Self {
            term,
            position,
            paragraph,
        }
    }
    pub fn term(&self) -> &str {
        &self.term
    }
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn paragraph(&self) -> &StyledText {
        &self.paragraph
    }
}
