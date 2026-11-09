//! 文体の手本（作品フォルダの `references/` に置いた参考文）。
//!
//! 何を手本にするかは、そこに置くかどうかで書き手が決める。本文を自動では手本にしない
//! （同じ原稿の中に、作者が書いた部分と AI と一緒に進めた部分が混ざることがあるため）。

use std::ops::Range;

use crate::error::Result;
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::stages::Stage;

/// 閉じかっこ。参考文の表記の癖（`？ 」`）を真似させないよう、この前の空白を除く。
const CLOSING_BRACKETS: [char; 6] = ['」', '』', '）', ')', '】', '〉'];

/// 読める参考文を、パスの順で返す（表記の癖を整えたもの。ファイルは変えない）。
/// UTF-8 のテキストとして読めないファイルは、気づけるように知らせて飛ばす。
pub(crate) fn load(stage: &Stage<'_>) -> Result<Vec<String>> {
    let mut references = Vec::new();
    for file in stage.project.reference_files()? {
        match file.text {
            Some(text) if !text.content.trim().is_empty() => references.push(tidy(&text.content)),
            Some(_) => {}
            None => notice(
                stage.caller.sink(),
                NoticeLevel::Warning,
                format!(
                    "{} は UTF-8 のテキストとして読めないため、文体の手本に使いません。",
                    file.path
                ),
            ),
        }
    }
    Ok(references)
}

/// LLM に真似させたくない表記の癖を除く。行末の空白と、閉じかっこの前の空白だけで、ほかの表記は書き手のまま残す。
fn tidy(text: &str) -> String {
    text.lines().map(tidy_line).collect::<Vec<_>>().join("\n")
}

fn tidy_line(line: &str) -> String {
    let mut tidied = String::with_capacity(line.len());
    for character in line.trim_end().chars() {
        if CLOSING_BRACKETS.contains(&character) {
            let kept = tidied.trim_end().len();
            // 行頭の字下げは閉じかっこの前でも残す
            if kept > 0 {
                tidied.truncate(kept);
            }
        }
        tidied.push(character);
    }
    tidied
}

/// 文体の手本として渡す抜粋。予算 `total_chars` を参考文で均等に分ける（短くて余った分はほかへ回す）。
/// 毎回同じ部分を選ぶ（結果を比べられるように、乱数は使わない）。並びは参考文の順。
pub(crate) fn excerpts(references: &[String], total_chars: usize) -> Vec<String> {
    let lengths: Vec<usize> = references
        .iter()
        .map(|reference| reference.chars().count())
        .collect();
    let mut shortest_first: Vec<usize> = (0..references.len()).collect();
    shortest_first.sort_by_key(|&index| lengths[index]);
    let mut shares = vec![0; references.len()];
    let mut remaining = total_chars;
    for (finished, &index) in shortest_first.iter().enumerate() {
        let share = (remaining / (references.len() - finished)).min(lengths[index]);
        shares[index] = share;
        remaining -= share;
    }
    references
        .iter()
        .zip(shares)
        .map(|(reference, share)| passage(reference, share))
        .filter(|excerpt| !excerpt.is_empty())
        .collect()
}

/// 文体ガイドの見本に使う抜粋。最初の参考文から引く（書き手がファイル名で順番を決められる）。
pub(crate) fn sample(references: &[String], max_chars: usize) -> Option<String> {
    references
        .first()
        .map(|reference| passage(reference, max_chars))
        .filter(|excerpt| !excerpt.is_empty())
}

/// 地の文と会話の両方が最も多く入る、`max_chars` 字以内の連続した部分（段落の途中では切らない）。
/// ファイルの先頭から機械的に取ると、書き出しの説明だけに偏るため。段落の間の空行も書かれたまま残す
/// （段落や場面の間の取り方も、手本の一部なので）。
fn passage(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    // 先頭の段落の字下げ（全角空白）は残す
    let text = text.trim_matches('\n');
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let lines: Vec<Line<'_>> = text.lines().map(Line::new).collect();
    let Some(window) = best_window(&lines, max_chars) else {
        // どの段落も長すぎて窓に入らないときは、先頭から切る
        return excerpt::head(text, max_chars);
    };
    lines[without_blank_edges(&lines, window)]
        .iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 参考文の 1 行。
struct Line<'a> {
    text: &'a str,
    /// 改行を含めた字数。
    chars: usize,
    kind: LineKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Narration,
    Dialogue,
    Blank,
}

impl<'a> Line<'a> {
    fn new(text: &'a str) -> Self {
        let trimmed = text.trim_start();
        let kind = if trimmed.is_empty() {
            LineKind::Blank
        } else if trimmed.starts_with(['「', '『']) {
            LineKind::Dialogue
        } else {
            LineKind::Narration
        };
        Self {
            text,
            chars: text.chars().count() + 1,
            kind,
        }
    }
}

/// 窓の中の字数。
#[derive(Default)]
struct Tally {
    total: usize,
    narration: usize,
    dialogue: usize,
}

impl Tally {
    fn add(&mut self, line: &Line<'_>) {
        self.total += line.chars;
        match line.kind {
            LineKind::Narration => self.narration += line.chars,
            LineKind::Dialogue => self.dialogue += line.chars,
            LineKind::Blank => {}
        }
    }

    fn remove(&mut self, line: &Line<'_>) {
        self.total -= line.chars;
        match line.kind {
            LineKind::Narration => self.narration -= line.chars,
            LineKind::Dialogue => self.dialogue -= line.chars,
            LineKind::Blank => {}
        }
    }

    fn balance(&self) -> usize {
        self.narration.min(self.dialogue)
    }
}

/// 窓の中の、地の文と会話の少ないほうの字数が最も多い窓。同じなら前のもの（毎回同じ部分を選ぶため）。
fn best_window(lines: &[Line<'_>], max_chars: usize) -> Option<Range<usize>> {
    let mut best: Option<(usize, Range<usize>)> = None;
    let mut end = 0;
    let mut tally = Tally::default();
    for start in 0..lines.len() {
        if end < start {
            end = start;
            tally = Tally::default();
        }
        while end < lines.len() && tally.total + lines[end].chars <= max_chars {
            tally.add(&lines[end]);
            end += 1;
        }
        if end > start {
            if best
                .as_ref()
                .is_none_or(|(best_balance, _)| tally.balance() > *best_balance)
            {
                best = Some((tally.balance(), start..end));
            }
            tally.remove(&lines[start]);
        }
    }
    best.map(|(_, window)| window)
}

/// 窓の両端の空行を除く（抜粋の前後に空行は要らない）。
fn without_blank_edges(lines: &[Line<'_>], mut window: Range<usize>) -> Range<usize> {
    while window.start < window.end && lines[window.start].kind == LineKind::Blank {
        window.start += 1;
    }
    while window.start < window.end && lines[window.end - 1].kind == LineKind::Blank {
        window.end -= 1;
    }
    window
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn tidying_removes_spaces_at_line_ends_and_before_closing_brackets_only() {
        assert_eq!(
            tidy("　朝が来た。　\n「本当？ 」と彼は言った。  \n「え　』"),
            "　朝が来た。\n「本当？」と彼は言った。\n「え』"
        );
    }

    #[test]
    fn tidying_keeps_the_indent_of_a_line_that_starts_with_a_closing_bracket() {
        assert_eq!(tidy("　」"), "　」");
    }

    #[test]
    fn a_short_reference_is_used_whole() {
        assert_eq!(passage("　雨。\n「はい」", 100), "　雨。\n「はい」");
    }

    #[test]
    fn the_passage_with_both_narration_and_dialogue_is_chosen_over_the_opening() {
        let narration_only = "　地の文だけの書き出し。\n".repeat(20);
        let mixed = "　地の文。\n「会話の文です」\n".repeat(5);
        let text = format!("{narration_only}{mixed}");

        let chosen = passage(&text, 60);

        assert!(chosen.contains("会話の文です"), "{chosen}");
        assert!(chosen.contains("地の文。"), "{chosen}");
        assert!(!text.starts_with(chosen.as_str()), "{chosen}");
        assert!(chosen.chars().count() <= 60);
    }

    #[test]
    fn blank_lines_inside_the_passage_are_kept_but_not_at_its_edges() {
        let text = "　地の文。\n「会話です」\n\n".repeat(10);

        let chosen = passage(&text, 60);

        assert!(chosen.contains("\n\n"), "{chosen:?}");
        assert!(
            !chosen.starts_with('\n') && !chosen.ends_with('\n'),
            "{chosen:?}"
        );
        assert!(chosen.chars().count() <= 60, "{chosen:?}");
    }

    #[test]
    fn the_same_passage_is_chosen_every_time() {
        let text = "　地の文。\n「会話」\n".repeat(30);
        assert_eq!(passage(&text, 40), passage(&text, 40));
    }

    #[test]
    fn a_paragraph_longer_than_the_budget_falls_back_to_the_head() {
        let text = "長い。".repeat(50);
        let chosen = passage(&text, 30);
        assert!(chosen.chars().count() <= 40, "{chosen}");
        assert!(chosen.starts_with("長い"), "{chosen}");
    }

    #[test]
    fn the_budget_is_split_evenly_and_what_a_short_reference_leaves_goes_to_the_others() {
        let references = vec!["短い。".to_owned(), "　長い文章の段落。\n".repeat(100)];

        let chosen = excerpts(&references, 100);

        assert_eq!(chosen[0], "短い。");
        assert!(chosen[1].chars().count() > 50, "{}", chosen[1]);
        assert!(chosen[1].chars().count() <= 97, "{}", chosen[1]);
    }

    #[test]
    fn no_budget_means_no_excerpts() {
        assert_eq!(excerpts(&["　雨。".to_owned()], 0).len(), 0);
    }
}
