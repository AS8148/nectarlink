// SPDX-License-Identifier: GPL-3.0-or-later
//! The app's language: the system's or the one chosen in Settings.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offered_languages_are_kept() {
        assert!(is_known("") && is_known("zh_CN") && is_known("en"));
        assert!(!is_known("xx"));
    }
}
