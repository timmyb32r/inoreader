use crate::{Definition, GlossaryError, MarkKind, StyledText, TextMark};
use ego_tree::NodeRef;
use scraper::{Html, Node};

/// Named `channel_definitions_v1` projection. HTML entities are decoded by the
/// HTML parser once; raw HTML remains authoritative in the observation record.
pub fn styled_html(html: &str) -> Result<StyledText, GlossaryError> {
    let doc = Html::parse_fragment(html);
    let mut builder = HtmlText::default();
    builder.walk(doc.tree.root())?;
    StyledText::new(builder.text, builder.marks)
}

#[derive(Default)]
struct HtmlText {
    text: String,
    units: usize,
    marks: Vec<TextMark>,
}
impl HtmlText {
    fn push(&mut self, value: &str) {
        self.text.push_str(value);
        self.units += value.encode_utf16().count();
    }
    fn boundary(&mut self) {
        if !self.text.is_empty() {
            if !self.text.ends_with('\n') {
                self.push("\n");
            }
            if !self.text.ends_with("\n\n") {
                self.push("\n");
            }
        }
    }
    fn walk(&mut self, node: NodeRef<'_, Node>) -> Result<(), GlossaryError> {
        let mut style = None;
        let mut block = false;
        match node.value() {
            Node::Text(text) => {
                self.push(text);
                return Ok(());
            }
            Node::Element(el) => {
                let name = el.name();
                block = matches!(name, "p" | "div" | "li" | "blockquote" | "h1" | "h2" | "h3");
                if block {
                    self.boundary();
                }
                style = match name {
                    "br" => {
                        self.push("\n");
                        return Ok(());
                    }
                    "b" | "strong" => Some(MarkKind::Bold),
                    "i" | "em" => Some(MarkKind::Italic),
                    "u" => Some(MarkKind::Underline),
                    "s" | "del" | "strike" => Some(MarkKind::Strike),
                    "code" => Some(MarkKind::Code),
                    "pre" => Some(MarkKind::Pre),
                    "blockquote" => Some(MarkKind::Quote),
                    "a" => Some(MarkKind::Link {
                        url: el.attr("href").ok_or(GlossaryError::Formatting)?.to_owned(),
                    }),
                    "span"
                        if el.has_class("tg-spoiler", scraper::CaseSensitivity::CaseSensitive) =>
                    {
                        Some(MarkKind::Spoiler)
                    }
                    "html" | "p" | "div" | "span" | "li" | "ol" | "ul" | "h1" | "h2" | "h3"
                    | "tg-emoji" => None,
                    _ => return Err(GlossaryError::Unsupported),
                };
            }
            Node::Comment(_) => return Ok(()),
            _ => {}
        }
        let start = self.units;
        for child in node.children() {
            self.walk(child)?;
        }
        if let Some(style) = style {
            if start != self.units {
                self.marks.push(TextMark {
                    start,
                    end: self.units,
                    style,
                });
            }
        }
        if block {
            self.boundary();
        }
        Ok(())
    }
}

pub fn definitions(text: &StyledText) -> Result<Vec<Definition>, GlossaryError> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < text.text().len() {
        // Blank-line runs delimit paragraphs; no source paragraph bytes change.
        while text.text().as_bytes().get(start) == Some(&b'\n') {
            start += 1;
        }
        if start == text.text().len() {
            break;
        }
        let end = text.text()[start..]
            .find("\n\n")
            .map_or(text.text().len(), |n| start + n);
        let units = text.text()[..start].encode_utf16().count();
        let size = text.text()[start..end].encode_utf16().count();
        if let Some(definition) = parse_paragraph(text.slice(units, units + size)?, units)? {
            result.push(definition)
        }
        start = end;
    }
    Ok(result)
}

pub(crate) fn parse_paragraph(
    paragraph: StyledText,
    position: usize,
) -> Result<Option<Definition>, GlossaryError> {
    let text = paragraph.text();
    let mut ranges: Vec<_> = paragraph
        .marks()
        .iter()
        .filter(|m| m.style == MarkKind::Bold)
        .collect();
    ranges.sort_by_key(|m| m.start);
    let mut bold_end = 0;
    for mark in ranges {
        if mark.start > bold_end {
            break;
        }
        bold_end = bold_end.max(mark.end);
    }

    if bold_end == 0 {
        return Ok(None);
    }
    for (at, ch) in text.char_indices() {
        if !matches!(ch, '-' | '–' | '—') || at == 0 {
            continue;
        }
        let before = &text[..at];
        let Some(space) = before.chars().next_back() else {
            continue;
        };
        if !matches!(space, ' ' | '\u{a0}') {
            continue;
        }
        let Some(after) = text[at + ch.len_utf8()..].chars().next() else {
            continue;
        };
        if !matches!(after, ' ' | '\u{a0}') {
            continue;
        }
        let term = &before[..before.len() - space.len_utf8()];
        if term.is_empty()
            || term.chars().next().is_some_and(char::is_whitespace)
            || term.chars().next_back().is_some_and(char::is_whitespace)
            || term.contains('\n')
            || term.encode_utf16().count() > bold_end
        {
            continue;
        }
        if text[at + ch.len_utf8() + after.len_utf8()..]
            .trim()
            .is_empty()
        {
            return Ok(None);
        }
        return Ok(Some(Definition::parsed(
            term.to_owned(),
            position,
            paragraph,
        )));
    }
    Ok(None)
}
