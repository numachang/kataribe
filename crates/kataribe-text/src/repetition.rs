//! 作品全体で使いすぎている表現の検出。
//!
//! LLM に続きを書かせると、前のシーンの決まり文句（「嵐の咆哮」など）を何度も繰り返しがちになる。
//! 書き終えた本文から繰り返し現れる表現を機械的に抜き出し、次の生成で避けさせるのに使う。

use std::collections::HashMap;

use crate::ruby::{self, is_kanji};

/// 数える表現の長さ（文字数）の範囲。
const MIN_PHRASE_CHARS: usize = 3;
const MAX_PHRASE_CHARS: usize = 10;
/// 表現に含まれていなければならない、仮名以外（漢字・カタカナ）の文字の数。
const MIN_CONTENT_CHARS: usize = 2;
/// 長い表現に含まれる短い表現は、長い表現の回数のこの倍率を超えて現れるときだけ別に数える
/// （「嵐の咆哮」を数えたら、その一部として数えられただけの「咆哮」は返さない）。
const CONTAINED_RATIO: usize = 2;

/// [`overused_phrases`] の挙動を決めるオプション。
#[derive(Debug, Clone, Copy)]
pub struct OverusedOptions<'a> {
    /// この回数以上現れた表現を「使いすぎ」とする。
    pub min_count: usize,
    /// 返す表現の最大数（目立つものから）。
    pub max_phrases: usize,
    /// 数えない語（人物名など、繰り返し出てきて当然のもの）。これを含む表現と、これの一部である表現は返さない。
    pub ignored_words: &'a [String],
}

/// 繰り返し現れた表現と、その回数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatedPhrase {
    pub phrase: String,
    pub count: usize,
}

/// `text` の中で `min_count` 回以上現れる表現を、目立つもの（回数 × 長さの大きいもの）から返す。
///
/// 対象は漢字かカタカナを含み、ひらがなで始まったり終わったりしない 3〜10 字の表現。
/// 「ていた」のような文法的な言い回しや、助詞をまたいだだけの断片は数えない。
/// ルビの記法は読みを除いてから数える。
pub fn overused_phrases(text: &str, options: &OverusedOptions<'_>) -> Vec<RepeatedPhrase> {
    let mut candidates: Vec<RepeatedPhrase> = count_content_phrases(&ruby::to_plain(text))
        .into_iter()
        .filter(|(phrase, count)| {
            *count >= options.min_count && !is_ignored(phrase, options.ignored_words)
        })
        .map(|(phrase, count)| RepeatedPhrase { phrase, count })
        .collect();

    // 長いものから採り、それに含まれるだけの短い表現は除く
    candidates.sort_by(|a, b| {
        char_len(b)
            .cmp(&char_len(a))
            .then(b.count.cmp(&a.count))
            .then(a.phrase.cmp(&b.phrase))
    });
    let mut accepted: Vec<RepeatedPhrase> = Vec::new();
    for candidate in candidates {
        let is_part_of_longer = accepted.iter().any(|longer| {
            longer.phrase.contains(&candidate.phrase)
                && candidate.count <= longer.count * CONTAINED_RATIO
        });
        if !is_part_of_longer {
            accepted.push(candidate);
        }
    }

    accepted.sort_by(|a, b| {
        prominence(b)
            .cmp(&prominence(a))
            .then(a.phrase.cmp(&b.phrase))
    });
    accepted.truncate(options.max_phrases);
    accepted
}

/// 句読点・括弧・空白などで区切った範囲の中で、内容のある表現の出現回数を数える。
fn count_content_phrases(plain: &str) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for segment in plain.split(|c: char| !is_japanese_letter(c)) {
        let chars: Vec<char> = segment.chars().collect();
        for length in MIN_PHRASE_CHARS..=MAX_PHRASE_CHARS.min(chars.len()) {
            for window in chars.windows(length) {
                if is_content_phrase(window) {
                    *counts.entry(window.iter().collect()).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

fn is_content_phrase(chars: &[char]) -> bool {
    let (Some(&first), Some(&last)) = (chars.first(), chars.last()) else {
        return false;
    };
    let content_chars = chars.iter().filter(|&&c| !is_hiragana(c)).count();
    !is_hiragana(first) && !is_hiragana(last) && content_chars >= MIN_CONTENT_CHARS
}

fn is_ignored(phrase: &str, ignored_words: &[String]) -> bool {
    ignored_words
        .iter()
        .filter(|word| !word.is_empty())
        .any(|word| phrase.contains(word.as_str()) || word.contains(phrase))
}

fn char_len(phrase: &RepeatedPhrase) -> usize {
    phrase.phrase.chars().count()
}

fn prominence(phrase: &RepeatedPhrase) -> usize {
    phrase.count * char_len(phrase)
}

fn is_hiragana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{309F}')
}

/// 表現の一部になる文字（ひらがな・カタカナ・長音記号・漢字）。
fn is_japanese_letter(c: char) -> bool {
    is_hiragana(c) || matches!(c, '\u{30A1}'..='\u{30FA}' | 'ー') || is_kanji(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn phrases(text: &str, ignored_words: &[String]) -> Vec<(String, usize)> {
        overused_phrases(
            text,
            &OverusedOptions {
                min_count: 3,
                max_phrases: 5,
                ignored_words,
            },
        )
        .into_iter()
        .map(|found| (found.phrase, found.count))
        .collect()
    }

    #[test]
    fn detects_a_phrase_repeated_across_scenes() {
        let text = [
            "　嵐の咆哮が屋根を叩いた。",
            "　凛は耳を澄ませた。",
            "　嵐の咆哮の合間に足音が聞こえる。",
            "　廊下は冷えていた。",
            "　再び嵐の咆哮が窓を震わせた。",
            "　やがて嵐の咆哮は遠のいた。",
        ]
        .join("\n");

        assert_eq!(phrases(&text, &[]), vec![("嵐の咆哮".to_owned(), 4)]);
    }

    #[test]
    fn grammatical_hiragana_phrases_are_not_counted() {
        let text = "　雨が降っていた。風が吹いていた。灯りが揺れていた。扉が軋んでいた。\n";

        assert_eq!(phrases(text, &[]), vec![]);
    }

    #[test]
    fn phrases_with_character_names_are_not_counted() {
        let text = "　霧島凛は耳を澄ませた。霧島凛は息を止めた。霧島凛は首を傾げた。\n";
        let names = vec!["霧島".to_owned(), "凛".to_owned()];

        assert_eq!(phrases(text, &names), vec![]);
    }

    #[test]
    fn phrases_below_the_threshold_are_not_counted() {
        let text = "　不協和音が響いた。遠くで不協和音が鳴った。\n";

        assert_eq!(phrases(text, &[]), vec![]);
    }

    #[test]
    fn ruby_readings_are_removed_before_counting() {
        let text = "　|咆哮《ほうこう》の音。咆哮の音。|咆哮《ほうこう》の音。\n";

        assert_eq!(phrases(text, &[]), vec![("咆哮の音".to_owned(), 3)]);
    }

    #[test]
    fn the_most_prominent_phrases_come_first_up_to_the_limit() {
        let text = [
            "　静かな湖面が揺れた。",
            "　静かな湖面を見た。",
            "　静かな湖面に映る。",
            "　不協和音が響いた。",
            "　不協和音を聞いた。",
            "　不協和音に震えた。",
            "　不協和音は止んだ。",
            "　不協和音と雨。",
            "　不協和音も消えた。",
        ]
        .concat();
        let found = overused_phrases(
            &text,
            &OverusedOptions {
                min_count: 3,
                max_phrases: 1,
                ignored_words: &[],
            },
        );

        assert_eq!(
            found,
            vec![RepeatedPhrase {
                phrase: "不協和音".to_owned(),
                count: 6
            }]
        );
    }
}
