-- Move document reasoning from application-global settings into each AI profile.
ALTER TABLE ai_provider_profiles
ADD COLUMN reasoning_effort TEXT NOT NULL DEFAULT 'low'
CHECK(reasoning_effort IN (
    'automatic',
    'none',
    'minimal',
    'low',
    'medium',
    'high',
    'xhigh',
    'max'
));

UPDATE ai_provider_profiles
SET reasoning_effort = COALESCE(
    (
        SELECT CASE json_extract(value_json, '$')
            WHEN 'automatic' THEN 'automatic'
            WHEN 'none' THEN 'none'
            WHEN 'minimal' THEN 'minimal'
            WHEN 'low' THEN 'low'
            WHEN 'medium' THEN 'medium'
            WHEN 'high' THEN 'high'
            WHEN 'xhigh' THEN 'xhigh'
            WHEN 'max' THEN 'max'
            ELSE 'low'
        END
        FROM app_settings
        WHERE key = 'aiReasoningEffort'
    ),
    'low'
);

DELETE FROM app_settings
WHERE key = 'aiReasoningEffort';
