//! 手元のテキスト原稿を 1 つの章として末尾に取り込む（LLM は使わない）。
//!
//! 空行をシーンの区切りとする。本文は書かれたまま取り込む（字下げ・記号を整形しない）。短すぎる塊は
//! 場面の書き出しの一文だけを独立させたものなどで、シーンにならないので、次のシーンにつなげる。

use std::ops::Range;

use kataribe_project::{Project, normalize_text};
use kataribe_text::count::count_chars;

use super::chapters;
use super::plan::StructurePlan;
use crate::error::{EngineError, Result};

/// 1 シーンとして扱う塊の、最小の字数の既定値。
pub const DEFAULT_MIN_SCENE_CHARS: usize = 200;

/// 取り込む原稿。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManuscriptImport {
    /// 作る章の題。
    pub chapter_title: String,
    /// 原稿の全文。BOM と CRLF は取り込むときに正規化する。
    pub text: String,
    /// これより短い塊は、次のシーンにつなげる。
    pub min_scene_chars: usize,
}

/// 原稿を章として取り込む変更案を作る。作品フォルダは書き換えない。
pub fn plan_manuscript_import(
    project: &Project,
    import: &ManuscriptImport,
) -> Result<StructurePlan> {
    let text = normalize_text(&import.text);
    let scenes: Vec<&str> = split_into_scenes(&text, import.min_scene_chars)
        .into_iter()
        .map(|range| &text[range])
        .collect();
    if scenes.is_empty() {
        return Err(EngineError::InvalidInput(
            "取り込む本文がありません。".into(),
        ));
    }
    Ok(chapters::add_with_manuscript(project, &import.chapter_title, &scenes)?.made_for(project))
}

/// シーンごとの本文の範囲。空のシーンは作らない。
fn split_into_scenes(text: &str, min_scene_chars: usize) -> Vec<Range<usize>> {
    let mut scenes: Vec<Range<usize>> = Vec::new();
    let mut joined_start = None;
    let mut last_end = 0;
    for block in blocks_of(text) {
        let start = joined_start.take().unwrap_or(block.start);
        last_end = block.end;
        if count_chars(&text[start..block.end]) < min_scene_chars {
            joined_start = Some(start);
        } else {
            scenes.push(start..block.end);
        }
    }
    // 最後が短い塊なら、次のシーンが無いので前のシーンにつなげる
    if let Some(start) = joined_start {
        match scenes.last_mut() {
            Some(last) => last.end = last_end,
            None => scenes.push(start..last_end),
        }
    }
    scenes
}

/// 空行で区切った塊の範囲（前後の空行と、最後の行の改行は含まない）。
/// 全角空白だけの行も空行とみなす（字下げの空白だけが残った行があるため）。
fn blocks_of(text: &str) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut current: Option<Range<usize>> = None;
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        let line_end = line_start + line.len();
        if line.trim().is_empty() {
            blocks.extend(current.take());
        } else {
            let content_end = line_end - usize::from(line.ends_with('\n'));
            let block_start = current.as_ref().map_or(line_start, |block| block.start);
            current = Some(block_start..content_end);
        }
        line_start = line_end;
    }
    blocks.extend(current);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn scenes_of(text: &str, min_scene_chars: usize) -> Vec<&str> {
        split_into_scenes(text, min_scene_chars)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn blank_lines_separate_scenes_however_many_there_are() {
        assert_eq!(
            scenes_of("　一つ目。\n\n　二つ目。\n\n\n\n　三つ目。\n", 1),
            vec!["　一つ目。", "　二つ目。", "　三つ目。"]
        );
    }

    #[test]
    fn a_line_of_only_full_width_spaces_counts_as_blank() {
        assert_eq!(
            scenes_of("　一つ目。\n　\n　二つ目。\n", 1),
            vec!["　一つ目。", "　二つ目。"]
        );
    }

    #[test]
    fn a_short_block_joins_the_next_scene_with_its_blank_lines_as_written() {
        assert_eq!(
            scenes_of("書き出しの一文。\n\n\n　長い本文が続く。\n", 9),
            vec!["書き出しの一文。\n\n\n　長い本文が続く。"]
        );
    }

    #[test]
    fn a_short_last_block_joins_the_previous_scene() {
        assert_eq!(
            scenes_of("　長い本文が続く。\n\n締め。\n", 8),
            vec!["　長い本文が続く。\n\n締め。"]
        );
    }

    #[test]
    fn several_short_blocks_in_a_row_join_until_they_reach_the_length() {
        assert_eq!(
            scenes_of("一。\n\n二。\n\n　長い本文が続く。\n", 8),
            vec!["一。\n\n二。\n\n　長い本文が続く。"]
        );
    }

    #[test]
    fn text_shorter_than_the_minimum_still_becomes_one_scene() {
        assert_eq!(scenes_of("短い。\n", 100), vec!["短い。"]);
    }

    #[test]
    fn spaces_inside_a_scene_are_kept_as_written() {
        assert_eq!(
            scenes_of("　字下げの行。　\n「台詞」\n", 1),
            vec!["　字下げの行。　\n「台詞」"]
        );
    }

    #[test]
    fn empty_text_has_no_scenes() {
        assert_eq!(scenes_of("\n\n　\n", 1), Vec::<&str>::new());
    }
}
