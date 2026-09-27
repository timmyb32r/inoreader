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
    article: &'a str,
    emitted: usize,
    content: String,
    limit: usize,
}

impl<'a> VerifiedSegments<'a> {
    pub fn new(article: &'a str, limit: usize) -> Self {
        Self {
            raw: String::new(),
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
        let trimmed = self.raw.trim_start();
        // The envelope permits arbitrary JSON whitespace but no other fields.
        let Some(after_object) = trimmed.strip_prefix('{') else {
            return Ok(None);
        };
        let Some(after_key) = after_object.trim_start().strip_prefix("\"segments\"") else {
            return Ok(None);
        };
        let Some(after_colon) = after_key.trim_start().strip_prefix(':') else {
            return Ok(None);
        };
        let Some(mut rest) = after_colon.trim_start().strip_prefix('[') else {
            return Ok(None);
        };
        let mut parsed = Vec::new();
        loop {
            rest = rest.trim_start();
            if rest.starts_with(']') || rest.is_empty() {
                break;
            }
            let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<Segment>();
            match stream.next() {
                Some(Ok(segment)) => {
                    parsed.push(segment);
                    rest = &rest[stream.byte_offset()..];
                    rest = rest.trim_start();
                    if let Some(next) = rest.strip_prefix(',') {
                        rest = next;
                    } else {
                        break;
                    }
                }
                Some(Err(error)) if error.is_eof() => break,
                _ => return Err(AiError::Protocol),
            }
        }
        let changed = parsed.len() > self.emitted;
        for segment in parsed.iter().skip(self.emitted) {
            let rendered = render_segment(segment, self.article)?;
            if !self.content.is_empty() {
                self.content.push_str("\n\n");
            }
            self.content.push_str(&rendered);
        }
        self.emitted = parsed.len();
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
