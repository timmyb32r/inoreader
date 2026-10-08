use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AiError, CostRates, GenerationMode};

/// Raw deployment configuration. `validate` runs before any worker/key operation;
/// an unapproved prompt never permits generation. Limits reject, never truncate.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    pub daily_limit_usd: String,

    pub automatic_schedule: AutomaticScheduleConfig,

    pub automatic_summaries: bool,

    /// Maximum unique articles admitted from a newly subscribed source archive.
    pub initial_articles: u32,

    /// User-enabled full-stream glossary extraction, under the same AI budget.
    #[serde(default)]
    pub automatic_terms: bool,

    pub automatic_attempts: u32,

    pub automatic_retry_seconds: u64,

    pub prompt_approved: bool,

    pub prompt_path: String,

    pub prompt_version: String,

    pub review: ReviewConfig,

    pub enabled_accounts: Vec<Uuid>,

    pub encryption_key_file_env: String,

    pub generation_mode: GenerationMode,

    pub context_tokens: usize,

    pub framing_tokens_per_message: usize,

    pub framing_tokens_base: usize,

    pub max_output_tokens: usize,

    pub max_input_bytes: usize,

    pub max_message_bytes: usize,

    pub max_response_bytes: usize,

    pub request_timeout_seconds: u64,

    pub connect_timeout_seconds: u64,

    pub workers: usize,

    pub poll_milliseconds: u64,

    pub lease_seconds: u64,

    /// Maximum expired jobs recovered per transaction and work class.
    pub recovery_batch: u32,

    pub models: crate::ModelRates,
}

impl AiConfig {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.prompt_version.is_empty()
            || !self
                .prompt_version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            || self.prompt_path.is_empty()
            || self.encryption_key_file_env.is_empty()
            || !self
                .encryption_key_file_env
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self.context_tokens == 0
            || self.framing_tokens_per_message == 0
            || self.framing_tokens_base == 0
            || self.max_output_tokens == 0
            || self.max_output_tokens >= self.context_tokens
            || self.max_input_bytes == 0
            || self.max_message_bytes == 0
            || self.max_response_bytes == 0
            || self.workers == 0
            || self.recovery_batch == 0
            || self.poll_milliseconds == 0
            || self.connect_timeout_seconds == 0
            || self.connect_timeout_seconds > self.request_timeout_seconds
            || self
                .request_timeout_seconds
                .checked_mul(2)
                .is_none_or(|n| self.lease_seconds <= n)
            || self.lease_seconds > i64::MAX as u64
            || self.request_timeout_seconds.checked_mul(1000).is_none()
            || self
                .framing_tokens_base
                .checked_add(self.max_output_tokens)
                .is_none_or(|v| v >= self.context_tokens)
        {
            return Err(AiError::Configuration);
        }
        self.automatic_schedule.validate()?;
        crate::SpendReservation::new(
            "1".into(),
            self.daily_limit_usd.clone(),
            crate::SpendMode::Summary,
        )?;
        if self.automatic_attempts == 0
            || self.initial_articles == 0
            || self.initial_articles > i32::MAX as u32
            || self.automatic_retry_seconds == 0
            || self.automatic_retry_seconds > i64::MAX as u64
        {
            return Err(AiError::Configuration);
        }
        self.review.validate(self.context_tokens)?;
        if self
            .framing_tokens_base
            .checked_add(self.review.max_output_tokens)
            .is_none_or(|n| n >= self.context_tokens)
        {
            return Err(AiError::Configuration);
        }
        Ok(())
    }
}

/// Validated execution policy; raw configuration cannot instantiate a worker.
#[derive(Clone)]
pub struct AiPolicy {
    config: AiConfig,

    prompt: String,

    review_prompt: String,
}

impl AiPolicy {
    pub fn new(config: AiConfig, prompt: String, review_prompt: String) -> Result<Self, AiError> {
        config.validate()?;
        if config.prompt_approved && (prompt.trim().is_empty() || review_prompt.trim().is_empty()) {
            return Err(AiError::Configuration);
        }
        Ok(Self {
            config,
            prompt,
            review_prompt,
        })
    }
    pub fn config(&self) -> &AiConfig {
        &self.config
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn cost_rates(&self) -> &CostRates {
        &self.config.models.flash
    }
    pub fn review_snapshot(&self) -> Result<ReviewSnapshot, AiError> {
        Ok(ReviewSnapshot {
            system_prompt: format!("{}\n{}", self.review_prompt, crate::REVIEW_TRANSPORT),
            prompt_version: self.config.review.prompt_version.clone(),
            model: crate::DeepSeekModel::Flash.id().into(),
            generation_mode: self.config.review.generation_mode,
            max_output_tokens: self.config.review.max_output_tokens,
            cost_rates: self.config.models.flash.clone(),
        })
    }
    pub fn allowed(&self, account: Uuid) -> bool {
        self.config.enabled_accounts.contains(&account)
    }
    pub fn generation_allowed(&self, account: Uuid) -> Result<(), AiError> {
        if !self.allowed(account) {
            return Err(AiError::Unavailable);
        }
        if !self.config.prompt_approved {
            return Err(AiError::PromptPending);
        }
        Ok(())
    }
}

/// Raw second-request settings; validation belongs to AiPolicy construction.
/// Both prompts share the explicit approval gate. A review is always required
/// for initial summaries; there is deliberately no silent single-call fallback.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewConfig {
    pub prompt_path: String,

    pub prompt_version: String,

    pub generation_mode: GenerationMode,

    pub max_output_tokens: usize,
}
impl ReviewConfig {
    fn validate(&self, context: usize) -> Result<(), AiError> {
        if self.prompt_path.is_empty()
            || self.max_output_tokens == 0
            || self.max_output_tokens >= context
            || [&self.prompt_version].iter().any(|s| {
                s.is_empty()
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            })
        {
            return Err(AiError::Configuration);
        }
        Ok(())
    }
}

/// Private stored prompt/model/settings: every paid boundary revalidates its
/// actual full input. Deserializing a record never itself authorizes execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewSnapshot {
    pub system_prompt: String,
    pub prompt_version: String,
    pub model: String,
    pub generation_mode: GenerationMode,
    pub max_output_tokens: usize,
    pub cost_rates: CostRates,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputLimits {
    pub context_tokens: usize,

    pub framing_tokens_per_message: usize,

    pub framing_tokens_base: usize,

    pub max_input_bytes: usize,

    pub max_response_bytes: usize,
}
impl From<&AiConfig> for InputLimits {
    fn from(c: &AiConfig) -> Self {
        Self {
            context_tokens: c.context_tokens,
            framing_tokens_per_message: c.framing_tokens_per_message,
            framing_tokens_base: c.framing_tokens_base,
            max_input_bytes: c.max_input_bytes,
            max_response_bytes: c.max_response_bytes,
        }
    }
}

/// Explicit deployment policy for automatic calls only. Beijing calendar dates;
/// peak windows are DeepSeek's Mon–Fri 09:00–12:00 and 14:00–18:00 (UTC+8),
/// start inclusive/end exclusive. Holidays and weekends are off-peak. After the
/// verified calendar expires, automatic calls pause until configuration is updated.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomaticScheduleConfig {
    pub pause_peak_hours: bool,
    pub holiday_calendar_valid_through: chrono::NaiveDate,
    pub public_holidays: Vec<chrono::NaiveDate>,
}
impl AutomaticScheduleConfig {
    pub fn validate(&self) -> Result<(), AiError> {
        let mut dates = std::collections::HashSet::new();
        if self
            .public_holidays
            .iter()
            .any(|day| *day > self.holiday_calendar_valid_through || !dates.insert(*day))
        {
            return Err(AiError::Configuration);
        }
        Ok(())
    }
}
