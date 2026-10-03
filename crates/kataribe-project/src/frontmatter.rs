//! YAML front matter を持つ Markdown の分解・合成。
//!
//! ファイルの先頭が `---` の行で始まる場合、次の `---` の行までを YAML として解釈し、
//! 残りを本文として扱う。front matter を持たないファイルは `parse_optional` で扱える。

use serde::Serialize;
use serde::de::DeserializeOwned;

/// front matter の YAML、または `kataribe.yaml` のような単独の YAML の解析・生成に関するエラー。
#[derive(Debug, thiserror::Error)]
pub enum YamlError {
    /// YAML の解析に失敗した。
    #[error("{0}")]
    Parse(String),
    /// `---` で始まっているのに、閉じの `---` が見つからなかった。
    #[error("front matter が閉じられていません（閉じの '---' が見つかりません）")]
    UnterminatedFrontMatter,
    /// front matter（`---` で始まる YAML）が無かった。
    #[error("front matter がありません（'---' で始まる YAML が必要です）")]
    MissingFrontMatter,
    /// YAML の生成に失敗した。
    #[error("YAML を書き出せませんでした: {0}")]
    Render(String),
}

/// front matter（YAML のメタデータ）と本文からなる文書。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<M> {
    /// YAML front matter を解釈した値。
    pub meta: M,
    /// front matter を除いた本文。
    pub body: String,
}

/// テキストを front matter 付き文書として解析する。
///
/// 先頭に front matter が無い場合はエラーになる。front matter が無くてもよい場合は
/// [`parse_optional`] を使う。
pub fn parse<M: DeserializeOwned>(text: &str) -> Result<Document<M>, YamlError> {
    match split_front_matter(text)? {
        Some(split) => {
            let meta = parse_yaml_with_line_offset(split.yaml, split.yaml_start_line)?;
            Ok(Document {
                meta,
                body: split.body.to_string(),
            })
        }
        None => Err(YamlError::MissingFrontMatter),
    }
}

/// テキストを front matter 付き文書として解析する。front matter が無ければ `M::default()` を使う。
pub fn parse_optional<M: DeserializeOwned + Default>(text: &str) -> Result<Document<M>, YamlError> {
    match split_front_matter(text)? {
        Some(split) => {
            let meta = parse_yaml_with_line_offset(split.yaml, split.yaml_start_line)?;
            Ok(Document {
                meta,
                body: split.body.to_string(),
            })
        }
        None => Ok(Document {
            meta: M::default(),
            body: text.to_string(),
        }),
    }
}

/// 文書を front matter 付きのテキストとして書き出す。
pub fn render<M: Serialize>(doc: &Document<M>) -> Result<String, YamlError> {
    let yaml = render_yaml(&doc.meta)?;
    let mut out = String::with_capacity(yaml.len() + doc.body.len() + 8);
    out.push_str("---\n");
    out.push_str(&yaml);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(&doc.body);
    Ok(out)
}

/// front matter の部分を書かれたまま残し、本文だけを `body` に差し替えたテキストを返す。
///
/// YAML を解釈し直さないので、コメント・項目の順番・引用符やブロック表記も変わらない。
/// front matter の判定は [`parse`] と同じで、無ければ [`YamlError::MissingFrontMatter`]、
/// 閉じが無ければ [`YamlError::UnterminatedFrontMatter`] になる。
pub fn replace_body(text: &str, body: &str) -> Result<String, YamlError> {
    let split = split_front_matter(text)?.ok_or(YamlError::MissingFrontMatter)?;
    let mut out = String::with_capacity(split.head.len() + body.len() + 1);
    out.push_str(split.head);
    // 閉じの `---` がファイルの最後の行（改行なし）だったとき、本文が同じ行に続かないようにする
    if !split.head.ends_with('\n') && !body.is_empty() {
        out.push('\n');
    }
    out.push_str(body);
    Ok(out)
}

/// front matter を伴わない、単独の YAML ドキュメントを解析する（`kataribe.yaml` 用）。
pub(crate) fn parse_yaml<M: DeserializeOwned>(text: &str) -> Result<M, YamlError> {
    parse_yaml_with_line_offset(text, 1)
}

/// front matter を伴わない、単独の YAML ドキュメントを生成する（`kataribe.yaml` 用）。
pub(crate) fn render_yaml<M: Serialize>(value: &M) -> Result<String, YamlError> {
    serde_saphyr::to_string(value).map_err(|err| YamlError::Render(err.to_string()))
}

/// `region_start_line` は、渡した `text` の 1 行目がファイル全体の何行目に当たるかを表す。
fn parse_yaml_with_line_offset<M: DeserializeOwned>(
    text: &str,
    region_start_line: u64,
) -> Result<M, YamlError> {
    serde_saphyr::from_str(text).map_err(|err| {
        let message = match err.location() {
            Some(location) => {
                let line = region_start_line + location.line() - 1;
                format!("{line} 行目: YAML を解析できません: {err}")
            }
            None => format!("YAML を解析できません: {err}"),
        };
        YamlError::Parse(message)
    })
}

struct FrontMatterSplit<'a> {
    /// 先頭から、閉じの `---` の行の終わりまで。
    head: &'a str,
    yaml: &'a str,
    body: &'a str,
    /// `yaml` の 1 行目が、元のテキスト全体で何行目に当たるか（1 始まり）。
    yaml_start_line: u64,
}

/// 先頭の `---` ～ `---` を切り出す。`---` で始まっていなければ `None`。
fn split_front_matter(text: &str) -> Result<Option<FrontMatterSplit<'_>>, YamlError> {
    let Some(after_first_marker) = text.strip_prefix("---") else {
        return Ok(None);
    };
    let first_line_rest = after_first_marker.split('\n').next().unwrap_or("");
    if !first_line_rest.trim_end_matches('\r').is_empty() {
        // 1 行目が "---" だけではない（例: "---foo"）ので front matter とは見なさない。
        return Ok(None);
    }
    let Some(newline_offset) = after_first_marker.find('\n') else {
        // "---" のみでファイルが終わっている。front matter は閉じられていない。
        return Err(YamlError::UnterminatedFrontMatter);
    };
    let rest = &after_first_marker[newline_offset + 1..];

    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Ok(Some(FrontMatterSplit {
                head: &text[..text.len() - body.len()],
                yaml,
                body,
                yaml_start_line: 2,
            }));
        }
        offset += line.len();
    }
    Err(YamlError::UnterminatedFrontMatter)
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Meta {
        title: Option<String>,
        #[serde(default, flatten)]
        extra: BTreeMap<String, serde_json::Value>,
    }

    #[test]
    fn parses_front_matter_and_body() {
        let text = "---\ntitle: 雨の匂い\n---\n本文here\n";
        let doc: Document<Meta> = parse(text).unwrap();
        assert_eq!(doc.meta.title.as_deref(), Some("雨の匂い"));
        assert_eq!(doc.body, "本文here\n");
    }

    #[test]
    fn parse_optional_defaults_when_no_front_matter() {
        let text = "front matter を持たない本文\n";
        let doc: Document<Meta> = parse_optional(text).unwrap();
        assert_eq!(doc.meta, Meta::default());
        assert_eq!(doc.body, text);
    }

    #[test]
    fn parse_requires_front_matter() {
        let text = "front matter を持たない本文\n";
        let result: Result<Document<Meta>, _> = parse(text);
        assert!(matches!(result, Err(YamlError::MissingFrontMatter)));
    }

    #[test]
    fn unterminated_front_matter_is_an_error() {
        let text = "---\ntitle: 開きっぱなし\n本文\n";
        let result: Result<Document<Meta>, _> = parse(text);
        assert!(matches!(result, Err(YamlError::UnterminatedFrontMatter)));
    }

    #[test]
    fn broken_yaml_reports_line_number() {
        let text = "---\ntitle: [\n---\n本文\n";
        let result: Result<Document<Meta>, _> = parse(text);
        let err = result.unwrap_err();
        let message = err.to_string();
        assert!(message.contains("行目"), "message was: {message}");
    }

    #[test]
    fn render_then_parse_roundtrips_known_and_unknown_fields() {
        let mut extra = BTreeMap::new();
        extra.insert("custom_note".to_string(), serde_json::json!("読者へのメモ"));
        extra.insert("priority".to_string(), serde_json::json!(3));
        let doc = Document {
            meta: Meta {
                title: Some("題名".to_string()),
                extra,
            },
            body: "一行目\n二行目\n".to_string(),
        };

        let rendered = render(&doc).unwrap();
        let parsed: Document<Meta> = parse(&rendered).unwrap();

        assert_eq!(parsed.meta.title, doc.meta.title);
        assert_eq!(parsed.meta.extra, doc.meta.extra);
        assert_eq!(parsed.body, doc.body);
    }

    #[test]
    fn replace_body_keeps_the_front_matter_exactly_as_written() {
        let front_matter = "---\n# 手で書いたコメント\nz_last: 'quoted'\ntitle:   雨の匂い\nnote: |\n  一行目\n  二行目\n---\n";
        let text = format!("{front_matter}古い本文\n");

        let replaced = replace_body(&text, "新しい本文\n").unwrap();

        assert_eq!(replaced, format!("{front_matter}新しい本文\n"));
    }

    #[test]
    fn replace_body_can_empty_the_body() {
        let replaced = replace_body("---\ntitle: 題\n---\n古い本文\n", "").unwrap();

        assert_eq!(replaced, "---\ntitle: 題\n---\n");
    }

    #[test]
    fn replace_body_does_not_join_the_body_to_a_closing_line_without_newline() {
        let replaced = replace_body("---\ntitle: 題\n---", "本文\n").unwrap();

        let document: Document<Meta> = parse(&replaced).unwrap();
        assert_eq!(document.meta.title.as_deref(), Some("題"));
        assert_eq!(document.body, "本文\n");
    }

    #[test]
    fn replace_body_requires_front_matter() {
        let result = replace_body("front matter の無い本文\n", "新しい本文");

        assert!(matches!(result, Err(YamlError::MissingFrontMatter)));
    }

    #[test]
    fn replace_body_rejects_an_unterminated_front_matter() {
        let result = replace_body("---\ntitle: 開きっぱなし\n本文\n", "新しい本文");

        assert!(matches!(result, Err(YamlError::UnterminatedFrontMatter)));
    }

    #[test]
    fn replace_body_does_not_validate_the_yaml() {
        // 本文だけを直すときに、壊れた YAML まで巻き込んで失敗しないように
        let replaced = replace_body("---\ntitle: [\n---\n古い\n", "新しい\n").unwrap();

        assert_eq!(replaced, "---\ntitle: [\n---\n新しい\n");
    }

    #[derive(Serialize)]
    struct WithNote {
        note: String,
    }

    #[test]
    fn multiline_string_is_rendered_as_block_scalar() {
        let rendered = render_yaml(&WithNote {
            note: "一行目\n二行目\n三行目".to_string(),
        })
        .unwrap();
        assert!(
            rendered.contains('|'),
            "expected block scalar, got: {rendered}"
        );
    }
}
