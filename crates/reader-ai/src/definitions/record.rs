use super::*;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DefinitionState {
    Queued,
    Generating,
    Completed { result: DefinitionResult },
    Failed { error: String },
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionsJob {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub article_id: Uuid,
    pub model: String,
    pub prompt_version: String,
    #[serde(flatten)]
    pub state: DefinitionState,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "RecordWire")]
pub struct DefinitionsRecord {
    owner: Uuid,
    input: DefinitionsInput,
    cost_rates: CostRates,
    job: DefinitionsJob,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWire {
    owner: Uuid,
    input: DefinitionsInput,
    cost_rates: CostRates,
    job: DefinitionsJob,
}
impl TryFrom<RecordWire> for DefinitionsRecord {
    type Error = AiError;
    fn try_from(v: RecordWire) -> Result<Self, Self::Error> {
        if v.job.model != v.input.model()
            || v.job.prompt_version != v.input.version()
            || v.owner.is_nil()
            || v.job.id.is_nil()
            || v.job.workspace_id.is_nil()
            || v.job.article_id.is_nil()
        {
            return Err(AiError::Protocol);
        }
        if let DefinitionState::Completed { result } = &v.job.state {
            result.check_source(v.input.snapshot())?;
        }
        Ok(Self {
            owner: v.owner,
            input: v.input,
            cost_rates: v.cost_rates,
            job: v.job,
        })
    }
}
impl DefinitionsRecord {
    pub fn new(
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        input: DefinitionsInput,
        cost_rates: CostRates,
    ) -> Result<Self, AiError> {
        let job = DefinitionsJob {
            id: Uuid::new_v4(),
            workspace_id: workspace,
            article_id: article,
            model: input.model().into(),
            prompt_version: input.version().into(),
            state: DefinitionState::Queued,
            usage: None,
        };
        RecordWire {
            owner,
            input,
            cost_rates,
            job,
        }
        .try_into()
    }
    pub fn owner(&self) -> Uuid {
        self.owner
    }
    pub fn input(&self) -> &DefinitionsInput {
        &self.input
    }
    pub fn job(&self) -> &DefinitionsJob {
        &self.job
    }
    pub fn cost_rates(&self) -> &CostRates {
        &self.cost_rates
    }
    pub fn mark_running(&mut self) -> Result<(), AiError> {
        if !matches!(self.job.state, DefinitionState::Queued) {
            return Err(AiError::Conflict);
        }
        self.job.state = DefinitionState::Generating;
        Ok(())
    }
    pub fn finish(&mut self, state: DefinitionState, usage: Option<Usage>) -> Result<(), AiError> {
        if !matches!(self.job.state, DefinitionState::Generating) {
            return Err(AiError::Conflict);
        }
        match &state {
            DefinitionState::Completed { result } => result.check_source(self.input.snapshot())?,
            DefinitionState::Failed { error } if !error.is_empty() => {}
            _ => return Err(AiError::Protocol),
        }
        self.job.state = state;
        self.job.usage = usage;
        Ok(())
    }
}
pub struct ClaimedDefinitions {
    pub record: DefinitionsRecord,
    pub lease: Uuid,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionsView {
    pub job: Option<DefinitionsJob>,
    pub channel: reader_glossary::ChannelStatus,
    pub known: Vec<reader_glossary::KnownDefinition>,
}
