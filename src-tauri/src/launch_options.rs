//! 起動時のコマンドライン引数。
//!
//! `--settings <PATH>` で、既定の場所（CLI と共有）の代わりに使う設定ファイルを指定できる。
//! CLI の同名のオプションと同じ意味で、設定や最近の作品を分けて使いたいとき（E2E テストなど）に使う。

use std::ffi::OsString;
use std::path::PathBuf;

const SETTINGS_FLAG: &str = "--settings";
const SETTINGS_FLAG_WITH_VALUE: &str = "--settings=";

/// 起動オプションの誤り。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum LaunchOptionError {
    /// `--settings` の後に設定ファイルのパスが無かった。
    #[error("起動オプション --settings の後に、設定ファイルのパスを指定してください。")]
    MissingSettingsPath,
    /// `--settings=` のパスが UTF-8 として読めなかった。
    #[error("起動オプション --settings のパスを読み取れません。")]
    UnreadableSettingsPath,
}

/// 起動時の引数（プログラム名を除く）から、設定ファイルの指定を取り出す。
/// `--settings <PATH>` と `--settings=<PATH>` を受け付け、複数あれば最初のものを使う。
/// ほかの引数は無視する（OS や `WebView2` が足す引数で起動に失敗しないように）。
///
/// 指定があるのにパスが無い場合はエラーにする。黙って既定の場所の設定を使うと、
/// 分けておきたかった設定（利用者の本物の設定など）を書き換えてしまうため。
pub(crate) fn settings_path_from_args<I>(args: I) -> Result<Option<PathBuf>, LaunchOptionError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(text) = arg.to_str() else {
            if arg.to_string_lossy().starts_with(SETTINGS_FLAG_WITH_VALUE) {
                return Err(LaunchOptionError::UnreadableSettingsPath);
            }
            continue;
        };
        if text == SETTINGS_FLAG {
            return match args.next() {
                Some(path) if !path.to_string_lossy().starts_with("--") => {
                    Ok(Some(PathBuf::from(path)))
                }
                _ => Err(LaunchOptionError::MissingSettingsPath),
            };
        }
        if let Some(path) = text.strip_prefix(SETTINGS_FLAG_WITH_VALUE) {
            return if path.is_empty() {
                Err(LaunchOptionError::MissingSettingsPath)
            } else {
                Ok(Some(PathBuf::from(path)))
            };
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<PathBuf>, LaunchOptionError> {
        settings_path_from_args(args.iter().map(OsString::from))
    }

    #[test]
    fn no_settings_flag_means_the_default_location() {
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(parse(&["--other", "value"]), Ok(None));
    }

    #[test]
    fn settings_path_follows_the_flag() {
        assert_eq!(
            parse(&["--settings", r"C:\e2e\settings.json"]),
            Ok(Some(PathBuf::from(r"C:\e2e\settings.json")))
        );
    }

    #[test]
    fn settings_path_can_be_joined_with_an_equals_sign() {
        assert_eq!(
            parse(&["--settings=portable.json"]),
            Ok(Some(PathBuf::from("portable.json")))
        );
    }

    #[test]
    fn unrelated_arguments_are_skipped() {
        assert_eq!(
            parse(&["--flag-from-webview2", "--settings", "a.json"]),
            Ok(Some(PathBuf::from("a.json")))
        );
    }

    #[test]
    fn flag_without_a_path_is_an_error() {
        for args in [
            &["--settings"][..],
            &["--settings="],
            &["--settings", "--flag-from-webview2"],
        ] {
            assert_eq!(
                parse(args),
                Err(LaunchOptionError::MissingSettingsPath),
                "{args:?}"
            );
        }
    }

    #[test]
    fn first_settings_flag_wins() {
        assert_eq!(
            parse(&["--settings", "first.json", "--settings", "second.json"]),
            Ok(Some(PathBuf::from("first.json")))
        );
    }
}
