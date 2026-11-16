//! Translatable feedback. The keys live in `assets/simplebackups/lang/en_us.json`; the English
//! line rides along as the component's fallback, so a vanilla client (this mod is server-only)
//! still reads it.
use std::collections::HashMap;
use std::sync::OnceLock;

use mod_api::Text;
use mod_api::text::TextContent;

static EN_US: OnceLock<HashMap<String, String>> = OnceLock::new();

fn en_us() -> &'static HashMap<String, String> {
    EN_US.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/simplebackups/lang/en_us.json"))
            .unwrap_or_default()
    })
}

/// A translatable line with its English fallback.
pub fn tr(key: &str, arguments: Vec<Text>) -> Text {
    Text::new(TextContent::Translatable {
        key: key.to_owned(),
        fallback: en_us().get(key).cloned(),
        arguments,
    })
}

/// A literal argument.
pub fn lit(value: impl ToString) -> Text {
    Text::literal(value.to_string())
}
