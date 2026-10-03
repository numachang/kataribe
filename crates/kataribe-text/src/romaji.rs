//! かな → ローマ字（ヘボン式を簡略にしたもの）。
//!
//! 人物の id や世界観の資料のファイル名を、読み（かな）から自動で作るために使う。
//! パスポートの表記に近く、`さとう` は `sato`、`しんいち` は `shinichi` になる
//! （LLM が作る id の `sato-kenji` と書き方をそろえるため）。

/// かな 1 文字を読んだ結果。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// かな 1 拍（拗音・外来音を含む）のローマ字。
    Syllable(String),
    /// 「ん」。
    Nasal,
    /// 「っ」。
    Geminate,
    /// 英数字。そのまま使う。
    Alphanumeric(char),
    /// 空白や中黒など、語の区切り。
    Separator,
    /// ローマ字にできない文字（漢字・記号・絵文字）。出力では区切りと同じに扱う。
    Unsupported,
}

/// かなを含む文字列をローマ字にする。
///
/// - かなは変換する（カタカナはひらがなに直してから）。英数字（全角も）はそのまま（全角は半角にする）。
/// - 空白・「・」「＝」「=」「‐」などは語の区切り（半角の空白 1 つ）にする。漢字・記号・絵文字は
///   ローマ字にできないので、区切りとして落とす。
/// - 「ん」は常に `n`（`b`・`m`・`p` の前でも `m` にしない）。促音は次の子音を重ねる
///   （`かっか` → `kakka`、`まっちゃ` → `matcha`）。語の末尾や母音・「ん」・区切りの前では落とす。
/// - 長音の「ー」「〜」は落とす。同じ語の中の `ou`・`oo` は `o`、`uu` は `u` にまとめる
///   （`さとう` → `sato`、`おおの` → `ono`）。`ei`・`ii`・`aa` はそのまま。
#[must_use]
pub fn to_romaji(text: &str) -> String {
    let mut writer = Writer::default();
    for piece in tokenize(text) {
        match piece {
            Piece::Syllable(romaji) => writer.syllable(&romaji),
            Piece::Nasal => writer.nasal(),
            Piece::Geminate => writer.geminate(),
            Piece::Alphanumeric(character) => writer.alphanumeric(character),
            Piece::Separator | Piece::Unsupported => writer.separator(),
        }
    }
    writer.finish()
}

/// 全体が、かな・英数字・区切りだけで書かれていて、ローマ字に直しても意味が失われないか。
///
/// 漢字を含む題は [`to_romaji`] で漢字が落ちて別の語になってしまうので、ローマ字を名前に使ってよいかの
/// 判断に使う。
#[must_use]
pub fn is_romanizable(text: &str) -> bool {
    tokenize(text)
        .iter()
        .all(|piece| *piece != Piece::Unsupported)
}

fn tokenize(text: &str) -> Vec<Piece> {
    let characters: Vec<char> = text.chars().map(normalize_character).collect();
    let mut pieces = Vec::with_capacity(characters.len());
    let mut index = 0;
    while index < characters.len() {
        let current = characters[index];
        let next = characters.get(index + 1).copied();
        if let Some((romaji, consumed)) = read_syllable(current, next) {
            pieces.push(Piece::Syllable(romaji));
            index += consumed;
            continue;
        }
        index += 1;
        match current {
            'ー' | '〜' | '～' => {}
            'っ' => pieces.push(Piece::Geminate),
            'ん' => pieces.push(Piece::Nasal),
            _ if current.is_ascii_alphanumeric() => pieces.push(Piece::Alphanumeric(current)),
            _ if is_separator(current) => pieces.push(Piece::Separator),
            _ => pieces.push(Piece::Unsupported),
        }
    }
    pieces
}

/// 全角英数字を半角に、カタカナをひらがなに直す。
fn normalize_character(character: char) -> char {
    let shifted = match character {
        '０'..='９' | 'Ａ'..='Ｚ' | 'ａ'..='ｚ' => u32::from(character) - 0xFEE0,
        '\u{30A1}'..='\u{30F6}' => u32::from(character) - 0x60,
        _ => return character,
    };
    char::from_u32(shifted).unwrap_or(character)
}

fn is_separator(character: char) -> bool {
    character.is_whitespace()
        || matches!(character, '・' | '＝' | '=' | '‐' | '-' | '－' | '_' | '＿')
}

/// `current` から始まる 1 拍を読む。小さい「ゃゅょぁぃぅぇぉ」が続けば合わせて読む。
/// 戻り値は、ローマ字と、読んだ文字数。
fn read_syllable(current: char, next: Option<char>) -> Option<(String, usize)> {
    if let Some(base) = base_syllable(current) {
        let combined = next.and_then(|small| combine(base, small));
        return Some(match combined {
            Some(romaji) => (romaji, 2),
            None => (base.to_owned(), 1),
        });
    }
    standalone_small_kana(current).map(|romaji| (romaji.to_owned(), 1))
}

/// 小さくない、かな 1 文字の読み。「ん」「っ」は含まない。
fn base_syllable(kana: char) -> Option<&'static str> {
    let romaji = match kana {
        'あ' => "a",
        'い' | 'ゐ' => "i",
        'う' => "u",
        'え' | 'ゑ' => "e",
        'お' | 'を' => "o",
        'か' => "ka",
        'き' => "ki",
        'く' => "ku",
        'け' => "ke",
        'こ' => "ko",
        'が' => "ga",
        'ぎ' => "gi",
        'ぐ' => "gu",
        'げ' => "ge",
        'ご' => "go",
        'さ' => "sa",
        'し' => "shi",
        'す' => "su",
        'せ' => "se",
        'そ' => "so",
        'ざ' => "za",
        'じ' | 'ぢ' => "ji",
        'ず' | 'づ' => "zu",
        'ぜ' => "ze",
        'ぞ' => "zo",
        'た' => "ta",
        'ち' => "chi",
        'つ' => "tsu",
        'て' => "te",
        'と' => "to",
        'だ' => "da",
        'で' => "de",
        'ど' => "do",
        'な' => "na",
        'に' => "ni",
        'ぬ' => "nu",
        'ね' => "ne",
        'の' => "no",
        'は' => "ha",
        'ひ' => "hi",
        'ふ' => "fu",
        'へ' => "he",
        'ほ' => "ho",
        'ば' => "ba",
        'び' => "bi",
        'ぶ' => "bu",
        'べ' => "be",
        'ぼ' => "bo",
        'ぱ' => "pa",
        'ぴ' => "pi",
        'ぷ' => "pu",
        'ぺ' => "pe",
        'ぽ' => "po",
        'ま' => "ma",
        'み' => "mi",
        'む' => "mu",
        'め' => "me",
        'も' => "mo",
        'や' => "ya",
        'ゆ' => "yu",
        'よ' => "yo",
        'ら' => "ra",
        'り' => "ri",
        'る' => "ru",
        'れ' => "re",
        'ろ' => "ro",
        'わ' => "wa",
        'ゔ' => "vu",
        _ => return None,
    };
    Some(romaji)
}

/// 前に結び付く相手が無いときの、小さいかなの読み。
fn standalone_small_kana(kana: char) -> Option<&'static str> {
    let romaji = match kana {
        'ぁ' => "a",
        'ぃ' => "i",
        'ぅ' => "u",
        'ぇ' => "e",
        'ぉ' => "o",
        'ゃ' => "ya",
        'ゅ' => "yu",
        'ょ' => "yo",
        'ゎ' => "wa",
        'ゕ' => "ka",
        'ゖ' => "ke",
        _ => return None,
    };
    Some(romaji)
}

/// 直前のかなに結び付く小さいかなが表す母音と、拗音（ゃゅょ）かどうか。
fn joining_small_kana(kana: char) -> Option<(char, bool)> {
    match kana {
        'ぁ' => Some(('a', false)),
        'ぃ' => Some(('i', false)),
        'ぅ' => Some(('u', false)),
        'ぇ' => Some(('e', false)),
        'ぉ' => Some(('o', false)),
        'ゃ' => Some(('a', true)),
        'ゅ' => Some(('u', true)),
        'ょ' => Some(('o', true)),
        _ => None,
    }
}

/// 1 拍（`base`）に小さいかな（`small`）が続くときの、合わせた読み。結び付かなければ `None`。
///
/// `きゃ` → `kya`、`しゃ` → `sha`、`ふぁ` → `fa`、`てぃ` → `ti`、`うぃ` → `wi`、`しぇ` → `she`。
/// 小さい母音が元の母音と同じなら（`きぃ`・`ふぅ`）、元の読みのまま。
fn combine(base: &str, small: char) -> Option<String> {
    let (vowel, is_glide) = joining_small_kana(small)?;
    let base_vowel = base.chars().last()?;
    let consonant = &base[..base.len() - base_vowel.len_utf8()];
    if !is_glide && vowel == base_vowel {
        return Some(base.to_owned());
    }
    let stem = if is_glide || base_vowel == 'i' {
        glide_stem(consonant)
    } else if consonant.is_empty() {
        match base_vowel {
            'u' => "w".to_owned(),
            _ => return None,
        }
    } else {
        consonant.to_owned()
    };
    Some(format!("{stem}{vowel}"))
}

/// 子音に「ゃ」の音（y）を足した語幹。`sh`・`ch`・`j` はそれ自体が拗音なので足さない
/// （`しゃ` は `shya` ではなく `sha`）。
fn glide_stem(consonant: &str) -> String {
    if matches!(consonant, "sh" | "ch" | "j") {
        consonant.to_owned()
    } else {
        format!("{consonant}y")
    }
}

/// 読んだ順にローマ字を書き出す。促音と長音の規則のために、直前の状態を持つ。
#[derive(Debug, Default)]
struct Writer {
    output: String,
    /// 「っ」を読んで、次の子音を重ねるのを待っている。
    pending_geminate: bool,
    /// 直前がかなの 1 拍のとき、そのローマ字の最後の文字（長音を `o`・`u` にまとめるため）。
    previous_vowel: Option<char>,
}

impl Writer {
    fn syllable(&mut self, romaji: &str) {
        if continues_long_vowel(self.previous_vowel, romaji) {
            self.pending_geminate = false;
            return;
        }
        if self.pending_geminate {
            push_doubled_consonant(&mut self.output, romaji);
            self.pending_geminate = false;
        }
        self.output.push_str(romaji);
        self.previous_vowel = romaji.chars().last();
    }

    fn nasal(&mut self) {
        self.pending_geminate = false;
        self.previous_vowel = None;
        self.output.push('n');
    }

    fn geminate(&mut self) {
        self.pending_geminate = true;
        self.previous_vowel = None;
    }

    fn alphanumeric(&mut self, character: char) {
        self.pending_geminate = false;
        self.previous_vowel = None;
        self.output.push(character);
    }

    fn separator(&mut self) {
        self.pending_geminate = false;
        self.previous_vowel = None;
        if !self.output.is_empty() && !self.output.ends_with(' ') {
            self.output.push(' ');
        }
    }

    fn finish(self) -> String {
        self.output.trim_end().to_owned()
    }
}

/// 直前の拍の母音に、同じ母音が続いて長音になる（`ou`・`oo`・`uu`）か。
fn continues_long_vowel(previous_vowel: Option<char>, romaji: &str) -> bool {
    matches!(
        (previous_vowel, romaji),
        (Some('o'), "o" | "u") | (Some('u'), "u")
    )
}

/// 促音のあとの 1 拍の頭の子音を重ねる。母音で始まる拍は重ねない。
/// `ch` は `cch` ではなく `tch` にする（`まっちゃ` → `matcha`）。
fn push_doubled_consonant(output: &mut String, romaji: &str) {
    let Some(first) = romaji.chars().next() else {
        return;
    };
    if matches!(first, 'a' | 'i' | 'u' | 'e' | 'o') {
        return;
    }
    output.push(if romaji.starts_with("ch") { 't' } else { first });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_romaji(input: &str, expected: &str) {
        assert_eq!(to_romaji(input), expected, "入力: {input}");
    }

    #[test]
    fn converts_basic_hiragana_with_hepburn_spelling() {
        assert_romaji("きりしま りん", "kirishima rin");
        assert_romaji("ちつふ", "chitsufu");
        assert_romaji("じぢづ", "jijizu");
        assert_romaji("ふじた", "fujita");
    }

    #[test]
    fn converts_wo_wi_and_we_to_their_vowel() {
        assert_romaji("を", "o");
        assert_romaji("ゐゑ", "ie");
    }

    #[test]
    fn converts_contracted_sounds() {
        assert_romaji("きゃ きゅ きょ", "kya kyu kyo");
        assert_romaji("しゃ しゅ しょ", "sha shu sho");
        assert_romaji("ちゃ ちゅ ちょ", "cha chu cho");
        assert_romaji("じゃ じゅ じょ", "ja ju jo");
        assert_romaji("ひゃ りょ", "hya ryo");
    }

    #[test]
    fn converts_sounds_borrowed_from_other_languages() {
        assert_romaji("ファ フィ フェ フォ", "fa fi fe fo");
        assert_romaji("ティ ディ", "ti di");
        assert_romaji("トゥ ドゥ", "tu du");
        assert_romaji("ウィ ウェ ウォ", "wi we wo");
        assert_romaji("ヴァ ヴィ ヴ ヴェ ヴォ", "va vi vu ve vo");
        assert_romaji("シェ ジェ チェ", "she je che");
        assert_romaji("ツァ", "tsa");
        assert_romaji("イェ", "ye");
    }

    #[test]
    fn small_vowel_matching_the_base_vowel_does_not_change_the_sound() {
        assert_romaji("きぃ", "ki");
        assert_romaji("ふぅ", "fu");
    }

    #[test]
    fn a_small_kana_with_nothing_to_join_is_read_alone() {
        assert_romaji("ぁ", "a");
        assert_romaji("ゃ", "ya");
        assert_romaji("ん ゃ", "n ya");
    }

    #[test]
    fn doubles_the_next_consonant_for_a_small_tsu() {
        assert_romaji("かっか", "kakka");
        assert_romaji("ざっし", "zasshi");
        assert_romaji("ぺっぱー", "peppa");
        assert_romaji("まっちゃ", "matcha");
        assert_romaji("ばっちり", "batchiri");
    }

    #[test]
    fn drops_a_small_tsu_at_the_end_or_before_a_vowel_nasal_or_separator() {
        assert_romaji("あっ", "a");
        assert_romaji("あっい", "ai");
        assert_romaji("あっん", "an");
        assert_romaji("あっ いう", "a iu");
    }

    #[test]
    fn writes_the_nasal_as_n_even_before_a_vowel_or_labial() {
        assert_romaji("しんいち", "shinichi");
        assert_romaji("けんじ", "kenji");
        assert_romaji("しんぱい", "shinpai");
        assert_romaji("かんな", "kanna");
        assert_romaji("りゅうせいん", "ryusein");
    }

    #[test]
    fn merges_long_vowels_within_a_word() {
        assert_romaji("さとう", "sato");
        assert_romaji("おおの", "ono");
        assert_romaji("ゆうこ", "yuko");
        assert_romaji("くうかい", "kukai");
        assert_romaji("りょうた", "ryota");
    }

    #[test]
    fn keeps_vowel_pairs_that_are_not_long_vowels() {
        assert_romaji("せいじ", "seiji");
        assert_romaji("いいだ", "iida");
        assert_romaji("あおい", "aoi");
        assert_romaji("あー", "a");
    }

    #[test]
    fn drops_the_long_vowel_mark() {
        assert_romaji("ラーメン", "ramen");
        assert_romaji("ケンジ〜", "kenji");
        assert_romaji("ケンジ～", "kenji");
    }

    #[test]
    fn does_not_merge_long_vowels_across_a_separator() {
        assert_romaji("さとう うみ", "sato umi");
    }

    #[test]
    fn converts_katakana_like_hiragana() {
        assert_romaji("キリシマ リン", "kirishima rin");
        assert_romaji("アリス", "arisu");
        assert_romaji("ヴァイオリン", "vaiorin");
        assert_romaji("ヵ", "ka");
    }

    #[test]
    fn keeps_ascii_alphanumerics_and_narrows_full_width_ones() {
        assert_romaji("Alice 7", "Alice 7");
        assert_romaji("ＡＢＣ１２３", "ABC123");
        assert_romaji("ａｂｃ", "abc");
        assert_romaji("りん２", "rin2");
    }

    #[test]
    fn treats_spaces_and_dots_as_word_breaks() {
        assert_romaji("りん　さとう", "rin sato");
        assert_romaji("りん・さとう", "rin sato");
        assert_romaji("りん＝さとう", "rin sato");
        assert_romaji("りん=さとう", "rin sato");
        assert_romaji("りん‐さとう", "rin sato");
        assert_romaji("りん   さとう", "rin sato");
    }

    #[test]
    fn drops_kanji_symbols_and_emoji_as_word_breaks() {
        assert_romaji("霧島 りん", "rin");
        assert_romaji("りん凛りん", "rin rin");
        assert_romaji("りん！？りん", "rin rin");
        assert_romaji("りん😀りん", "rin rin");
        assert_romaji("霧島 凛", "");
    }

    #[test]
    fn returns_an_empty_string_for_empty_or_blank_input() {
        assert_romaji("", "");
        assert_romaji("   ", "");
        assert_romaji("・", "");
    }

    #[test]
    fn has_no_leading_or_trailing_break() {
        assert_romaji("　りん　", "rin");
        assert_romaji("凛りん凛", "rin");
    }

    #[test]
    fn is_romanizable_only_for_kana_alphanumerics_and_separators() {
        assert!(is_romanizable("きりしま りん"));
        assert!(is_romanizable("ヴァイオリン・ソナタ"));
        assert!(is_romanizable("Chapter 1 ＡＢＣ"));
        assert!(is_romanizable("ラーメン〜"));
        assert!(is_romanizable(""));
        assert!(!is_romanizable("霧島 りん"));
        assert!(!is_romanizable("りん！"));
        assert!(!is_romanizable("りん😀"));
    }
}
