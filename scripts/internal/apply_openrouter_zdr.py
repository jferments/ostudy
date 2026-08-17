from __future__ import annotations

import hashlib
import re
from pathlib import Path


def read(path: str | Path) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str | Path, source: str) -> None:
    Path(path).write_text(source, encoding="utf-8")


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one target, found {count}: {old[:100]!r}")
    return source.replace(old, new, 1)


def add_struct_field(source: str, struct_name: str, anchor: str, replacement: str) -> str:
    pattern = re.compile(
        rf"(?P<head>(?:pub(?:\(crate\))?\s+)?struct\s+{re.escape(struct_name)}\s*\{{)"
        rf"(?P<body>[\s\S]*?)(?P<tail>\n\}})"
    )
    match = pattern.search(source)
    if match is None:
        raise SystemExit(f"missing struct {struct_name}")
    body = match.group("body")
    if "require_zero_data_retention" in body:
        return source
    if body.count(anchor) != 1:
        raise SystemExit(f"{struct_name}: expected one field anchor, found {body.count(anchor)}")
    body = body.replace(anchor, replacement, 1)
    return source[: match.start("body")] + body + source[match.end("body") :]


def matching_brace(source: str, opening: int) -> int:
    depth = 0
    quote: str | None = None
    escaped = False
    for index in range(opening, len(source)):
        char = source[index]
        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            continue
        if char in {'"', "'"}:
            quote = char
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    raise SystemExit("unterminated Rust block while adding privacy field")


def add_rust_literal_field(source: str, type_name: str) -> str:
    starts = [m.start() for m in re.finditer(rf"\b{re.escape(type_name)}\s*\{{", source)]
    for start in reversed(starts):
        line_start = source.rfind("\n", 0, start) + 1
        prefix = source[line_start:start]
        if "struct " in prefix:
            continue
        opening = source.find("{", start)
        closing = matching_brace(source, opening)
        body = source[opening + 1 : closing]
        if "require_zero_data_retention:" in body:
            continue
        matches = list(
            re.finditer(
                r"(?m)^(?P<indent>\s*)allow_insecure_http:\s*[^,\n]+,$",
                body,
            )
        )
        if not matches:
            continue
        field = matches[0]
        insertion = opening + 1 + field.end()
        indent = field.group("indent")
        source = (
            source[:insertion]
            + f"\n{indent}require_zero_data_retention: false,"
            + source[insertion:]
        )
    return source


# Profile data model, validation, SQL, and DTOs.
profiles_path = Path("src-tauri/src/ai/profiles.rs")
profiles = read(profiles_path)
for struct_name, anchor, replacement in (
    (
        "CreateAiProviderProfileInput",
        "\n    pub allow_insecure_http: bool,\n    pub request_timeout_ms: i64,",
        "\n    pub allow_insecure_http: bool,\n    #[serde(default)]\n    pub require_zero_data_retention: bool,\n    pub request_timeout_ms: i64,",
    ),
    (
        "UpdateAiProviderProfileInput",
        "\n    pub allow_insecure_http: bool,\n    pub request_timeout_ms: i64,",
        "\n    pub allow_insecure_http: bool,\n    #[serde(default)]\n    pub require_zero_data_retention: bool,\n    pub request_timeout_ms: i64,",
    ),
    (
        "AiProviderProfileDetailDto",
        "\n    pub allow_insecure_http: bool,\n    pub request_timeout_ms: i64,",
        "\n    pub allow_insecure_http: bool,\n    pub require_zero_data_retention: bool,\n    pub request_timeout_ms: i64,",
    ),
    (
        "ProfileSnapshot",
        "\n    pub allow_insecure_http: bool,\n    pub request_timeout_ms: i64,",
        "\n    pub allow_insecure_http: bool,\n    pub require_zero_data_retention: bool,\n    pub request_timeout_ms: i64,",
    ),
    (
        "StoredProfile",
        "\n    allow_insecure_http: bool,\n    request_timeout_ms: i64,",
        "\n    allow_insecure_http: bool,\n    require_zero_data_retention: bool,\n    request_timeout_ms: i64,",
    ),
    (
        "RawProfileRow",
        "\n    allow_insecure_http: i64,\n    request_timeout_ms: i64,",
        "\n    allow_insecure_http: i64,\n    require_zero_data_retention: i64,\n    request_timeout_ms: i64,",
    ),
):
    profiles = add_struct_field(profiles, struct_name, anchor, replacement)

for old, new, label in (
    (
        "            allow_insecure_http: row.allow_insecure_http != 0,\n            request_timeout_ms: row.request_timeout_ms,",
        "            allow_insecure_http: row.allow_insecure_http != 0,\n            require_zero_data_retention: row.require_zero_data_retention != 0,\n            request_timeout_ms: row.request_timeout_ms,",
        "raw profile conversion",
    ),
    (
        "            allow_insecure_http: profile.allow_insecure_http,\n            request_timeout_ms: profile.request_timeout_ms,",
        "            allow_insecure_http: profile.allow_insecure_http,\n            require_zero_data_retention: profile.require_zero_data_retention,\n            request_timeout_ms: profile.request_timeout_ms,",
        "profile snapshot conversion",
    ),
    (
        "        validate_endpoint(self.provider_kind, &self.base_url, self.allow_insecure_http)?;",
        "        validate_privacy_configuration(\n            self.provider_kind,\n            self.require_zero_data_retention,\n        )?;\n        validate_endpoint(self.provider_kind, &self.base_url, self.allow_insecure_http)?;",
        "snapshot privacy validation",
    ),
    (
        "    let next_endpoint = validate_endpoint(\n        current.provider_kind,\n        &input.base_url,\n        input.allow_insecure_http,\n    )?;",
        "    validate_privacy_configuration(\n        current.provider_kind,\n        input.require_zero_data_retention,\n    )?;\n    let next_endpoint = validate_endpoint(\n        current.provider_kind,\n        &input.base_url,\n        input.allow_insecure_http,\n    )?;",
        "update command privacy validation",
    ),
    (
        "    validate_provider_configuration(input.provider_kind, input.auth_mode)?;\n    let display_name = validate_display_name(input.display_name)?;",
        "    validate_provider_configuration(input.provider_kind, input.auth_mode)?;\n    validate_privacy_configuration(\n        input.provider_kind,\n        input.require_zero_data_retention,\n    )?;\n    let display_name = validate_display_name(input.display_name)?;",
        "create profile privacy validation",
    ),
    (
        "    validate_provider_configuration(provider_kind, input.auth_mode)?;\n    let display_name = validate_display_name(input.display_name)?;",
        "    validate_provider_configuration(provider_kind, input.auth_mode)?;\n    validate_privacy_configuration(\n        provider_kind,\n        input.require_zero_data_retention,\n    )?;\n    let display_name = validate_display_name(input.display_name)?;",
        "update profile privacy validation",
    ),
    (
        "                 model_id, auth_mode, enabled, allow_insecure_http,\n                 request_timeout_ms, credential_storage, credential_epoch, revision,",
        "                 model_id, auth_mode, enabled, allow_insecure_http,\n                 require_zero_data_retention, request_timeout_ms,\n                 credential_storage, credential_epoch, revision,",
        "profile insert columns",
    ),
    (
        "                 ?6, ?7, ?8, ?9,\n                 ?10, 'none', 0, 1,\n                 ?11, ?11, NULL",
        "                 ?6, ?7, ?8, ?9,\n                 ?10, ?11, 'none', 0, 1,\n                 ?12, ?12, NULL",
        "profile insert values",
    ),
    (
        "                bool_to_integer(input.allow_insecure_http),\n                input.request_timeout_ms,\n                now,",
        "                bool_to_integer(input.allow_insecure_http),\n                bool_to_integer(input.require_zero_data_retention),\n                input.request_timeout_ms,\n                now,",
        "profile insert params",
    ),
    (
        "                 allow_insecure_http = ?7,\n                 request_timeout_ms = ?8,\n                 updated_at = ?9,\n                 revision = revision + 1\n             WHERE id = ?10 AND revision = ?11 AND deleted_at IS NULL",
        "                 allow_insecure_http = ?7,\n                 require_zero_data_retention = ?8,\n                 request_timeout_ms = ?9,\n                 updated_at = ?10,\n                 revision = revision + 1\n             WHERE id = ?11 AND revision = ?12 AND deleted_at IS NULL",
        "profile update SQL",
    ),
    (
        "                bool_to_integer(input.allow_insecure_http),\n                input.request_timeout_ms,\n                now,\n                input.profile_id,",
        "                bool_to_integer(input.allow_insecure_http),\n                bool_to_integer(input.require_zero_data_retention),\n                input.request_timeout_ms,\n                now,\n                input.profile_id,",
        "profile update params",
    ),
):
    profiles = replace_once(profiles, old, new, label)

query_old = (
    "                    model_id, auth_mode, enabled, allow_insecure_http,\n"
    "                    request_timeout_ms, credential_storage, credential_epoch, revision,"
)
query_new = (
    "                    model_id, auth_mode, enabled, allow_insecure_http,\n"
    "                    require_zero_data_retention, request_timeout_ms,\n"
    "                    credential_storage, credential_epoch, revision,"
)
if profiles.count(query_old) != 2:
    raise SystemExit(f"profile SELECT columns: expected two targets, found {profiles.count(query_old)}")
profiles = profiles.replace(query_old, query_new)
profiles = replace_once(
    profiles,
    "        allow_insecure_http: row.get(7)?,\n        request_timeout_ms: row.get(8)?,\n        credential_storage: row.get(9)?,\n        credential_epoch: row.get(10)?,\n        revision: row.get(11)?,\n        created_at: row.get(12)?,\n        updated_at: row.get(13)?,",
    "        allow_insecure_http: row.get(7)?,\n        require_zero_data_retention: row.get(8)?,\n        request_timeout_ms: row.get(9)?,\n        credential_storage: row.get(10)?,\n        credential_epoch: row.get(11)?,\n        revision: row.get(12)?,\n        created_at: row.get(13)?,\n        updated_at: row.get(14)?,",
    "profile row positions",
)
profiles = replace_once(
    profiles,
    "        allow_insecure_http: profile.allow_insecure_http,\n        request_timeout_ms: profile.request_timeout_ms,",
    "        allow_insecure_http: profile.allow_insecure_http,\n        require_zero_data_retention: profile.require_zero_data_retention,\n        request_timeout_ms: profile.request_timeout_ms,",
    "profile detail DTO",
)
profiles = replace_once(
    profiles,
    "fn validate_display_name(value: String) -> Result<String, CommandError> {",
    "fn validate_privacy_configuration(\n    provider_kind: ProviderKind,\n    require_zero_data_retention: bool,\n) -> Result<(), CommandError> {\n    if require_zero_data_retention && provider_kind != ProviderKind::Openrouter {\n        return Err(CommandError::new(\n            \"invalid_ai_privacy_policy\",\n            \"Zero-data-retention routing is available only for OpenRouter profiles.\",\n        ));\n    }\n    Ok(())\n}\n\nfn validate_display_name(value: String) -> Result<String, CommandError> {",
    "privacy validation helper",
)
for type_name in ("ProfileSnapshot", "CreateAiProviderProfileInput", "UpdateAiProviderProfileInput"):
    profiles = add_rust_literal_field(profiles, type_name)
profiles = replace_once(
    profiles,
    "    #[test]\n    fn rejects_planned_provider_profiles() {",
    "    #[test]\n    fn openrouter_zero_data_retention_is_persisted_and_provider_scoped() {\n        let database = Database::open_in_memory().expect(\"database should open\");\n        let mut input = default_profile_input(ProviderKind::Openrouter);\n        input.operation_id = \"17171717171717171717171717171717\".to_string();\n        input.display_name = \"OpenRouter ZDR\".to_string();\n        input.model_id = \"openai/example\".to_string();\n        input.require_zero_data_retention = true;\n        let profile_id = database\n            .with_connection(|connection| create_profile(connection, input))\n            .expect(\"OpenRouter ZDR profile should save\");\n        let snapshot = database\n            .with_connection(|connection| load_profile_snapshot(connection, &profile_id))\n            .expect(\"saved profile should load\");\n        assert!(snapshot.require_zero_data_retention);\n\n        let mut invalid = default_profile_input(ProviderKind::LlamaCpp);\n        invalid.operation_id = \"18181818181818181818181818181818\".to_string();\n        invalid.display_name = \"Invalid local ZDR\".to_string();\n        invalid.require_zero_data_retention = true;\n        let error = database\n            .with_connection(|connection| create_profile(connection, invalid))\n            .expect_err(\"non-OpenRouter profiles must reject ZDR routing\");\n        assert_eq!(error.code, \"invalid_ai_privacy_policy\");\n    }\n\n    #[test]\n    fn rejects_planned_provider_profiles() {",
    "privacy persistence test",
)
write(profiles_path, profiles)

for path in Path("src-tauri/src").rglob("*.rs"):
    if path == profiles_path:
        continue
    source = read(path)
    updated = add_rust_literal_field(source, "ProfileSnapshot")
    if updated != source:
        write(path, updated)

migration_path = Path("src-tauri/migrations/0001_public_baseline.sql")
migration = read(migration_path)
migration = replace_once(
    migration,
    "    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),\n    request_timeout_ms",
    "    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),\n    require_zero_data_retention INTEGER NOT NULL DEFAULT 0\n                                CHECK(require_zero_data_retention IN (0, 1)),\n    request_timeout_ms",
    "public baseline privacy column",
)
write(migration_path, migration)

openrouter_path = Path("src-tauri/src/ai/providers/openrouter.rs")
openrouter = read(openrouter_path)
for old, new, label in (
    (
        "pub(super) fn text_probe_body(model_id: &str, messages: Value) -> Value {",
        "pub(super) fn text_probe_body(\n    model_id: &str,\n    messages: Value,\n    require_zero_data_retention: bool,\n) -> Value {",
        "text probe signature",
    ),
    (
        "pub(super) fn structured_probe_body(model_id: &str, echo: &str, schema: Value) -> Value {",
        "pub(super) fn structured_probe_body(\n    model_id: &str,\n    echo: &str,\n    schema: Value,\n    require_zero_data_retention: bool,\n) -> Value {",
        "structured probe signature",
    ),
    (
        "pub(super) fn provider_preferences(require_parameters: bool) -> Value {",
        "pub(super) fn provider_preferences(\n    require_parameters: bool,\n    require_zero_data_retention: bool,\n) -> Value {",
        "provider preferences signature",
    ),
    (
        '        "data_collection": "deny",\n        "allow_fallbacks": true',
        '        "data_collection": "deny",\n        "zdr": require_zero_data_retention,\n        "allow_fallbacks": true',
        "provider ZDR field",
    ),
):
    openrouter = replace_once(openrouter, old, new, label)
if openrouter.count('"provider": provider_preferences(true)') != 2:
    raise SystemExit("expected two OpenRouter provider-preference calls")
openrouter = openrouter.replace(
    '"provider": provider_preferences(true)',
    '"provider": provider_preferences(true, require_zero_data_retention)',
)
openrouter = replace_once(
    openrouter,
    '        let body = structured_probe_body("openai/example", "echo", json!({ "type": "object" }));',
    '        let body = structured_probe_body(\n            "openai/example",\n            "echo",\n            json!({ "type": "object" }),\n            true,\n        );',
    "structured probe test call",
)
openrouter = replace_once(
    openrouter,
    '        assert_eq!(\n            body.pointer("/provider/data_collection"),\n            Some(&json!("deny"))\n        );',
    '        assert_eq!(\n            body.pointer("/provider/data_collection"),\n            Some(&json!("deny"))\n        );\n        assert_eq!(body.pointer("/provider/zdr"), Some(&json!(true)));',
    "structured probe ZDR assertion",
)
openrouter = replace_once(
    openrouter,
    "    #[test]\n    fn midstream_style_errors_receive_stable_safe_categories() {",
    "    #[test]\n    fn provider_preferences_make_zdr_explicit_without_weakening_data_collection() {\n        let ordinary = provider_preferences(true, false);\n        assert_eq!(ordinary.pointer(\"/data_collection\"), Some(&json!(\"deny\")));\n        assert_eq!(ordinary.pointer(\"/zdr\"), Some(&json!(false)));\n\n        let strict = provider_preferences(true, true);\n        assert_eq!(strict.pointer(\"/data_collection\"), Some(&json!(\"deny\")));\n        assert_eq!(strict.pointer(\"/zdr\"), Some(&json!(true)));\n        assert_eq!(strict.pointer(\"/require_parameters\"), Some(&json!(true)));\n    }\n\n    #[test]\n    fn midstream_style_errors_receive_stable_safe_categories() {",
    "provider policy test",
)
write(openrouter_path, openrouter)

providers_path = Path("src-tauri/src/ai/providers.rs")
providers = read(providers_path)
for old, new, label in (
    (
        "pub(crate) fn openrouter_provider_preferences(require_parameters: bool) -> Value {\n    openrouter::provider_preferences(require_parameters)\n}",
        "pub(crate) fn openrouter_provider_preferences(\n    require_parameters: bool,\n    require_zero_data_retention: bool,\n) -> Value {\n    openrouter::provider_preferences(require_parameters, require_zero_data_retention)\n}",
        "exported preferences",
    ),
    (
        "            Self::Openrouter => openrouter::text_probe_body(&profile.model_id, messages),",
        "            Self::Openrouter => openrouter::text_probe_body(\n                &profile.model_id,\n                messages,\n                profile.require_zero_data_retention,\n            ),",
        "text probe routing",
    ),
    (
        "            Self::Openrouter => openrouter::structured_probe_body(&profile.model_id, echo, schema),",
        "            Self::Openrouter => openrouter::structured_probe_body(\n                &profile.model_id,\n                echo,\n                schema,\n                profile.require_zero_data_retention,\n            ),",
        "structured probe routing",
    ),
):
    providers = replace_once(providers, old, new, label)
providers = add_rust_literal_field(providers, "ProfileSnapshot")
write(providers_path, providers)

document_path = Path("src-tauri/src/ai/document_intelligence.rs")
document = read(document_path)
document = replace_once(
    document,
    '"provider": providers::openrouter_provider_preferences(true),',
    '"provider": providers::openrouter_provider_preferences(\n            true,\n            profile.require_zero_data_retention,\n        ),',
    "document-generation policy",
)
document = add_rust_literal_field(document, "ProfileSnapshot")
write(document_path, document)

probes_path = Path("src-tauri/src/ai/probes.rs")
probes = read(probes_path)
probes = replace_once(
    probes,
    "const PROVIDER_ADAPTER_REVISION: i64 = 3;",
    "const PROVIDER_ADAPTER_REVISION: i64 = 4;",
    "provider adapter revision",
)
write(probes_path, probes)

ipc_path = Path("src/ipc/aiProfiles.ts")
ipc = read(ipc_path)
anchor = "  allowInsecureHttp: boolean;\n  requestTimeoutMs: number;"
if ipc.count(anchor) != 2:
    raise SystemExit(f"frontend profile fields: expected two anchors, found {ipc.count(anchor)}")
ipc = ipc.replace(
    anchor,
    "  allowInsecureHttp: boolean;\n  requireZeroDataRetention?: boolean;\n  requestTimeoutMs: number;",
)
ipc = replace_once(
    ipc,
    "      allowInsecureHttp: input.allowInsecureHttp,\n      requestTimeoutMs: input.requestTimeoutMs,",
    "      allowInsecureHttp: input.allowInsecureHttp,\n      requireZeroDataRetention: input.requireZeroDataRetention ?? false,\n      requestTimeoutMs: input.requestTimeoutMs,",
    "frontend update payload",
)
write(ipc_path, ipc)

controller_path = Path("src/controllers/aiProfilesController.ts")
controller = read(controller_path)
controller = replace_once(
    controller,
    "      allowInsecureHttp: false,\n      requestTimeoutMs: provider.defaultTimeoutMs,",
    "      allowInsecureHttp: false,\n      requireZeroDataRetention: false,\n      requestTimeoutMs: provider.defaultTimeoutMs,",
    "new profile default",
)
controller = replace_once(
    controller,
    "    allowInsecureHttp: profile.allowInsecureHttp,\n    requestTimeoutMs: profile.requestTimeoutMs,",
    "    allowInsecureHttp: profile.allowInsecureHttp,\n    requireZeroDataRetention: profile.requireZeroDataRetention ?? false,\n    requestTimeoutMs: profile.requestTimeoutMs,",
    "saved profile draft",
)
write(controller_path, controller)

settings_path = Path("src/screens/settings/AiLlmSettings.tsx")
settings = read(settings_path)
settings = replace_once(
    settings,
    '  const isLlama = draft.providerKind === "llama_cpp";',
    '  const isLlama = draft.providerKind === "llama_cpp";\n  const isOpenRouter = draft.providerKind === "openrouter";',
    "OpenRouter discriminator",
)
settings = replace_once(
    settings,
    "        ) : null}\n      </div>\n\n      {!isLlama ? (",
    "        ) : null}\n        {isOpenRouter ? (\n          <label className={`${styles.switchRow} ${styles.fullWidth}`}>\n            <span>\n              <strong>Require zero data retention</strong>\n              <small>\n                Restrict OpenRouter routing to endpoints that declare zero data retention.\n                This can reduce endpoint availability.\n              </small>\n            </span>\n            <input\n              type=\"checkbox\"\n              checked={draft.requireZeroDataRetention ?? false}\n              onChange={(event) =>\n                controller.updateDraft({\n                  requireZeroDataRetention: event.currentTarget.checked,\n                })\n              }\n            />\n          </label>\n        ) : null}\n      </div>\n\n      {!isLlama ? (",
    "OpenRouter settings switch",
)
settings = replace_once(
    settings,
    "    draft.allowInsecureHttp !== selected.allowInsecureHttp ||\n    draft.requestTimeoutMs !== selected.requestTimeoutMs",
    "    draft.allowInsecureHttp !== selected.allowInsecureHttp ||\n    (draft.requireZeroDataRetention ?? false) !==\n      (selected.requireZeroDataRetention ?? false) ||\n    draft.requestTimeoutMs !== selected.requestTimeoutMs",
    "profile dirty comparison",
)
settings = replace_once(
    settings,
    "    allowInsecureHttp: selected.allowInsecureHttp,\n    requestTimeoutMs: selected.requestTimeoutMs,",
    "    allowInsecureHttp: selected.allowInsecureHttp,\n    requireZeroDataRetention: selected.requireZeroDataRetention ?? false,\n    requestTimeoutMs: selected.requestTimeoutMs,",
    "saved profile comparison",
)
settings = replace_once(
    settings,
    '    return "OpenRouter is a cloud routing provider. ostudy uses its fixed official API endpoint, requests only parameter-compatible endpoints for structured generation, and denies data-collecting endpoints by default.";',
    '    return "OpenRouter is a cloud routing provider. ostudy uses its fixed official API endpoint, requests only parameter-compatible endpoints for structured generation, denies data-collecting endpoints by default, and can require zero-data-retention routing per profile.";',
    "OpenRouter notice",
)
write(settings_path, settings)

ipc_test_path = Path("src/ipc/aiProfiles.test.ts")
ipc_test = read(ipc_test_path)
ipc_test = replace_once(
    ipc_test,
    "      allowInsecureHttp: true,\n      requestTimeoutMs: 120_000,",
    "      allowInsecureHttp: true,\n      requireZeroDataRetention: false,\n      requestTimeoutMs: 120_000,",
    "IPC test draft",
)
ipc_test = replace_once(
    ipc_test,
    "        allowInsecureHttp: true,\n        requestTimeoutMs: 120_000,",
    "        allowInsecureHttp: true,\n        requireZeroDataRetention: false,\n        requestTimeoutMs: 120_000,",
    "IPC expected payload",
)
write(ipc_test_path, ipc_test)

adr_path = Path("docs/decisions/0037-openrouter-provider-adapter.md")
adr = read(adr_path)
adr = replace_once(
    adr,
    "Structured document requests use strict JSON Schema plus `provider.require_parameters=true`. The default routing policy sets `provider.data_collection=deny` and allows compatible endpoint fallback. ostudy does not request OpenRouter plugins/context compression and never enables the provider's upstream-body debug echo. Streaming top-level errors abort the window and discard partial output.",
    "Structured document requests use strict JSON Schema plus `provider.require_parameters=true`. The default routing policy sets `provider.data_collection=deny` and allows compatible endpoint fallback. Each OpenRouter profile can additionally require `provider.zdr=true`; changing that setting increments the profile revision and invalidates older capability evidence. ostudy explicitly disables OpenRouter context compression because document windowing is owned locally, and it never enables the provider's upstream-body debug echo. Streaming top-level errors abort the window and discard partial output.",
    "OpenRouter ADR policy",
)
write(adr_path, adr)

guide_path = Path("docs/user-guide/ai-providers.md")
guide = read(guide_path)
guide = replace_once(
    guide,
    "- request timeout and enabled state;\n- capability evidence from the most recent successful verification;",
    "- request timeout and enabled state;\n- an optional OpenRouter-only zero-data-retention requirement;\n- capability evidence from the most recent successful verification;",
    "AI guide profile fields",
)
guide = replace_once(
    guide,
    "For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It does not enable provider-side prompt/body debugging or context-compression plugins. A stricter policy can reduce endpoint availability; the connection test reports that as a routing/capability failure rather than silently weakening the request.",
    "For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It explicitly disables provider-side context compression so document windows are not silently changed, and it never enables prompt/body debugging. Enable **Require zero data retention** on a profile to send `provider.zdr=true`; this stronger restriction can reduce endpoint availability, and the connection test reports that as a routing/capability failure rather than silently weakening the request.",
    "AI guide OpenRouter policy",
)
write(guide_path, guide)

digest = hashlib.sha256(migration_path.read_bytes()).hexdigest()
database_path = Path("src-tauri/src/database.rs")
database = read(database_path)
database, count = re.subn(
    r'(name: "public_baseline",\n\s*checksum: ")[0-9a-f]{64}("\s*,)',
    rf"\g<1>{digest}\g<2>",
    database,
    count=1,
)
if count != 1:
    raise SystemExit("could not update public baseline checksum")
write(database_path, database)
print(f"public baseline sha256={digest}")
