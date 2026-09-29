-- Preferences outlive key removal. Existing accounts deliberately default to
-- Flash/Flash; saved summaries and historical tariff snapshots are untouched.
CREATE TABLE ai_model_preferences (
    owner UUID PRIMARY KEY,
    document TEXT NOT NULL
);
-- Exact selection and rates for every newly admitted request. Old finished
-- requests retain their original chat snapshots and settled spending records.
CREATE TABLE ai_call_models (
    id UUID PRIMARY KEY,
    owner UUID NOT NULL,
    document TEXT NOT NULL
);

-- Explicit owner/day exceptions expire by their Moscow calendar date; no reset job.
CREATE TABLE ai_budget_overrides (
    owner UUID NOT NULL,
    day DATE NOT NULL,
    limit_usd NUMERIC NOT NULL CHECK(limit_usd > 0 AND limit_usd <> 'NaN'::numeric AND limit_usd <> 'Infinity'::numeric),
    PRIMARY KEY(owner, day)
);
