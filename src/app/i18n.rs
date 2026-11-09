//! Two-language UI text.
//!
//! Every string is carried inline next to the layout code that draws it, so a
//! label and its translation can never drift apart or get lost in a table:
//!
//! ```ignore
//! ui.label(app.t("查看", "View"));
//! ```
//!
//! Texts that need formatting keep their placeholders in both variants.

use super::state::Lang;

/// Picks the active language's variant of an inline pair.
pub fn t(lang: Lang, zh: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Zh => zh,
        Lang::En => en,
    }
}
