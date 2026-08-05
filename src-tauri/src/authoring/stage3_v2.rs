use std::collections::{BTreeMap, HashMap};

// Stage 3 v2 is a lexical continuation of the parent `authoring` module rather
// than an independent Rust module. Generated-card identity, template CRUD,
// structural rendering and compatibility normalization share private helpers
// through these `include!` calls. The ordering below is significant.
include!("stage3_v2/types_commands.rs");
include!("stage3_v2/generated_cards.rs");
include!("stage3_v2/projected_render.rs");
include!("stage3_v2/templates.rs");
include!("stage3_v2/template_update_wrapper.rs");
include!("stage3_v2/auto_template_layout.rs");
include!("stage3_v2/normalization.rs");
include!("stage3_v2/tests.rs");
include!("stage3_v2/asset_lifecycle_tests.rs");
