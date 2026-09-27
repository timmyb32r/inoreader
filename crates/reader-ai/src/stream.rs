use serde::Deserialize;

use crate::AiError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    kind: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    segments: Vec<Segment>,
}

/// Incrementally accepts complete JSON segments, never incomplete strings or
/// source quotations. Final parsing rejects trailing/malformed provider output.
pub(crate) struct VerifiedSegments<'a> {
    raw: String,
    cursor: usize,
    array_started: bool,
    array_finished: bool,
    object_start: Option<usize>,
    depth: usize,
    quoted: bool,
    escaped: bool,
    separator: bool,
    article: &'a str,
    emitted: usize,
    content: String,
    limit: usize,
}

impl<'a> VerifiedSegments<'a> {
    pub fn new(article: &'a str, limit: usize) -> Self {
        Self {
            raw: String::new(),
            cursor: 0,
            array_started: false,
            array_finished: false,
            object_start: None,
            depth: 0,
            quoted: false,
            escaped: false,
            separator: false,
            article,
            emitted: 0,
            content: String::new(),
            limit,
        }
    }
    pub fn push(&mut self, text: &str) -> Result<Option<&str>, AiError> {
        if self
            .raw
            .len()
            .checked_add(text.len())
            .ok_or(AiError::Protocol)?
            > self.limit
        {
            return Err(AiError::Protocol);
        }
        self.raw.push_str(text);
        let mut changed = false;
        // Scan every incoming byte once, including incomplete/escaped strings.
        // Deserialize only a newly closed object, never the growing prefix.
        while self.cursor < self.raw.len() {
            let position = self.cursor;
            let byte = self.raw.as_bytes()[position];
            self.cursor += 1;
            if self.array_finished {
                continue;
            }
            if self.quoted {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.quoted = false;
                }
                continue;
            }
            if byte == b'"' {
                if self.array_started && self.object_start.is_none() {
                    return Err(AiError::Protocol);
                }
                self.quoted = true;
                continue;
            }
            if !self.array_started {
                if byte == b'[' {
                    let header = format!("{}]}}", &self.raw[..self.cursor]);
                    let _: Envelope =
                        serde_json::from_str(&header).map_err(|_| AiError::Protocol)?;
                    self.array_started = true;
                }
                continue;
            }
            if self.object_start.is_none() {
                if byte.is_ascii_whitespace() {
                    continue;
                }
                if byte == b']' {
                    self.array_finished = true;
                    continue;
                }
                if self.separator {
                    if byte != b',' {
                        return Err(AiError::Protocol);
                    }
                    self.separator = false;
                    continue;
                }
                if byte != b'{' {
                    return Err(AiError::Protocol);
                }
                self.object_start = Some(position);
                self.depth = 1;
                continue;
            }
            if byte == b'{' {
                self.depth += 1;
            }
            if byte == b'}' {
                self.depth -= 1;
                if self.depth == 0 {
                    let start = self.object_start.take().ok_or(AiError::Protocol)?;
                    let segment: Segment = serde_json::from_str(&self.raw[start..self.cursor])
                        .map_err(|_| AiError::Protocol)?;
                    let rendered = render_segment(&segment, self.article)?;
                    if !self.content.is_empty() {
                        self.content.push_str("\n\n");
                    }
                    self.content.push_str(&rendered);
                    self.emitted += 1;
                    self.separator = true;
                    changed = true;
                }
            }
        }
        Ok(changed.then_some(self.content.as_str()))
    }
    pub fn finish(&self) -> Result<&str, AiError> {
        let envelope: Envelope = serde_json::from_str(&self.raw).map_err(|_| AiError::Protocol)?;
        if envelope.segments.is_empty() || envelope.segments.len() != self.emitted {
            return Err(AiError::Protocol);
        }
        for segment in &envelope.segments {
            render_segment(segment, self.article)?;
        }
        Ok(&self.content)
    }
    pub fn envelope(&self) -> &str {
        &self.raw
    }
}

fn render_segment(segment: &Segment, source: &str) -> Result<String, AiError> {
    if segment.content.is_empty() {
        return Err(AiError::Protocol);
    }
    match segment.kind.as_str() {
        "text" => {
            if segment
                .content
                .lines()
                .any(|line| line.trim_start().starts_with('>'))
            {
                return Err(AiError::Quote);
            }
            Ok(segment.content.clone())
        }
        "quote" => {
            if !source.contains(&segment.content) {
                return Err(AiError::Quote);
            }
            Ok(segment
                .content
                .split('\n')
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        _ => Err(AiError::Protocol),
    }
}
