-- Exact paid model output and its actual system prompt, including rejected replies.
-- No cascade: credentials/retention changes must not silently erase paid history.
CREATE TABLE ai_call_responses (
 id UUID PRIMARY KEY REFERENCES ai_spending(id),
 owner UUID NOT NULL,
 content TEXT NOT NULL,
 system_prompt TEXT NOT NULL
);
