// SPDX-License-Identifier: GPL-3.0-or-later
//! The app's language: the system's or the one chosen in Settings, for the
//! QML (Qt's translators) and for the text the app builds in Rust, through
//! [`tr!`]. `cargo xtask strings` collects every `tr!` text into
//! `desktop/app/i18n/rust_strings.h`, which `lupdate` reads with the QML, so
//! translators see them in the same `.ts` catalogue.

use std::path::{Path, PathBuf};

/// Languages the app speaks, as Settings offers them: a locale name ("" for
/// the system's) and how the language calls itself.
pub const LANGUAGES: &[(&str, &str)] = &[("", ""), ("en", "English"), ("zh_CN", "简体中文")];

/// Whether `language` is one [`LANGUAGES`] offers.
pub fn is_known(language: &str) -> bool {
    LANGUAGES.iter().any(|(code, _)| *code == language)
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// Uses `language` ("" for the system's) from now on, retranslating the
/// open windows. Call on the Qt thread, after the `QGuiApplication` exists.
pub fn install(language: &str) {
    let language = if is_known(language) { language } else { "" };
    let Some(dir) = exe_dir() else { return };
    let installed = crate::bridge::native::ffi::install_translator(&dir.to_string_lossy(), language);
    tracing::debug!(language, installed, "translator");
}

/// `text` (from `context`) in the app's language. Use [`tr!`], which also
/// fills in `%1`, `%2`... and marks the text for `cargo xtask strings`.
/// With no translation installed (or none for `text`), `text` itself.
pub fn translate(context: &str, text: &str) -> String {
    crate::bridge::native::ffi::translate(context, text)
}

/// Puts `args` in for `%1`, `%2`... in `text` (Qt's placeholders, which
/// translators may reorder). `%%` is a literal `%`.
pub fn fill(text: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('%') => {
                chars.next();
                out.push('%');
            }
            Some(d @ '1'..='9') => {
                chars.next();
                match args.get(d as usize - '1' as usize) {
                    Some(arg) => out.push_str(&arg.to_string()),
                    None => {
                        out.push('%');
                        out.push(d);
                    }
                }
            }
            _ => out.push('%'),
        }
    }
    out
}

/// Text the app shows, in the user's language: `tr!("Context", "Sent to %1", name)`.
/// The context and text must be literals (`cargo xtask strings` reads them).
macro_rules! tr {
    ($context:literal, $text:literal $(,)?) => {
        $crate::i18n::translate($context, $text)
    };
    ($context:literal, $text:literal, $($arg:expr),+ $(,)?) => {
        $crate::i18n::fill(&$crate::i18n::translate($context, $text), &[$(&$arg as &dyn std::fmt::Display),+])
    };
}
pub(crate) use tr;

/// A timeline entry's title in the app's language: the core names its
/// sessions in English ("Phone webcam"); anything else (a file's or a
/// person's name) stays as it is.
pub fn timeline_title(title: &str) -> String {
    match title {
        "Phone webcam" => tr!("Timeline", "Phone webcam"),
        "Screen mirroring" => tr!("Timeline", "Screen mirroring"),
        "Phone microphone" => tr!("Timeline", "Phone microphone"),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_fill_in_any_order() {
        assert_eq!(fill("Sent to %1", &[&"Pixel"]), "Sent to Pixel");
        assert_eq!(fill("%2 then %1", &[&"a", &"b"]), "b then a");
        assert_eq!(fill("100%% of %1", &[&3]), "100% of 3");
        // A placeholder without an argument stays, so the mistake shows.
        assert_eq!(fill("%1 and %2", &[&"x"]), "x and %2");
        assert_eq!(fill("50% off", &[]), "50% off");
    }

    #[test]
    fn tr_fills_its_arguments() {
        let name = "Pixel 9";
        assert_eq!(tr!("Tests", "Sent to %1", name), "Sent to Pixel 9");
        assert_eq!(tr!("Tests", "Done"), "Done");
    }

    #[test]
    fn only_offered_languages_are_kept() {
        assert!(is_known("") && is_known("zh_CN") && is_known("en"));
        assert!(!is_known("xx"));
    }
}
