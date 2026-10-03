//! 人物の名前の突き合わせ。
//!
//! シーンの視点人物・登場人物は名前の文字列で書かれる。生成も、人物を消すときの参照の確認も、
//! その文字列がどの人物を指すかを同じ規則で判断する。

use kataribe_project::Character;

/// 空白の有無を無視して名前が一致するか（「霧島 凛」と「霧島凛」を同一人物とみなす）。
pub(crate) fn same_person(left: &str, right: &str) -> bool {
    let squeeze = |name: &str| {
        name.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    squeeze(left) == squeeze(right)
}

/// `name`（空白で区切った姓や名のどれか）が、`character_name` の一部と一致するか。「凛」だけの書き方を拾う。
fn is_part_of_name(character_name: &str, name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && character_name.split_whitespace().any(|part| part == name)
}

/// シーンに書かれた `name` が、`character_name` の人物を指しているか。
/// 空白の違いと、「凛」のように姓や名だけの書き方を同一人物とみなす。空の名前はだれも指さない。
pub(crate) fn refers_to(character_name: &str, name: &str) -> bool {
    !name.trim().is_empty()
        && (same_person(character_name, name) || is_part_of_name(character_name, name))
}

/// 名前から人物を探す。名前が全体で一致する人物を優先し、無ければ姓や名だけの一致を探す。
pub(crate) fn find_character<'a>(characters: &'a [Character], name: &str) -> Option<&'a Character> {
    characters
        .iter()
        .find(|character| same_person(&character.meta.name, name))
        .or_else(|| {
            characters
                .iter()
                .find(|character| is_part_of_name(&character.meta.name, name))
        })
}

#[cfg(test)]
mod tests {
    use kataribe_project::{CharacterId, CharacterMeta};

    use super::*;

    fn character(id: &str, name: &str) -> Character {
        Character {
            id: CharacterId::new(id).unwrap(),
            meta: CharacterMeta {
                name: name.to_owned(),
                ..CharacterMeta::default()
            },
            body: String::new(),
        }
    }

    #[test]
    fn names_match_regardless_of_spacing() {
        assert!(same_person("霧島 凛", "霧島凛"));
        assert!(same_person("霧島　凛", "霧島 凛"));
        assert!(!same_person("霧島 凛", "霧島 蓮"));
    }

    #[test]
    fn a_family_or_given_name_alone_refers_to_the_person() {
        assert!(refers_to("霧島 凛", "凛"));
        assert!(refers_to("霧島 凛", "霧島"));
        assert!(refers_to("霧島 凛", "霧島凛"));
        assert!(!refers_to("霧島 凛", "蓮"));
        assert!(!refers_to("霧島 凛", "凛々"));
    }

    #[test]
    fn an_empty_name_refers_to_nobody() {
        assert!(!refers_to("霧島 凛", ""));
        assert!(!refers_to("霧島 凛", "  "));
        assert!(!refers_to("", ""));
    }

    #[test]
    fn find_character_prefers_a_full_name_match_over_a_partial_one() {
        let characters = [character("rin-sato", "佐藤 凛"), character("rin", "凛")];

        let found = find_character(&characters, "凛").unwrap();

        assert_eq!(found.id.as_str(), "rin");
    }

    #[test]
    fn find_character_falls_back_to_a_partial_match() {
        let characters = [character("rin", "霧島 凛")];

        assert_eq!(
            find_character(&characters, "凛").unwrap().id.as_str(),
            "rin"
        );
        assert!(find_character(&characters, "蓮").is_none());
        assert!(find_character(&characters, "").is_none());
    }
}
