//! 並べ替えの行き先（位置）の検証と、並べ替えそのもの。人物・章・シーンの並べ替えが共有する。

use crate::error::{EngineError, Result};

/// 一覧の `from` 番目の項目を、並べ替えたあとに `position` 番目（どちらも 0 始まり）に来るよう動かす。
///
/// 呼ぶ前に [`ensure_new_position`] で、`from`・`position` が一覧の範囲にあることを確かめておく。
pub(super) fn move_item<T>(items: &mut Vec<T>, from: usize, position: usize) {
    let item = items.remove(from);
    items.insert(position, item);
}

/// 項目を動かせる位置か確かめる。範囲の外と、今と同じ位置は、利用者が直せる入力の誤りとして知らせる。
///
/// 今と同じ位置を空の変更案にしないのは、何も起きない操作を成功として返すと、呼び出し側の数え間違いが
/// 見えなくなるため（画面は今と同じ位置を送らない）。`subject` は「人物「霧島 凛」」のような呼び方、
/// `unit` は「人物」「章」「シーン」のような数え方の名前。
pub(super) fn ensure_new_position(
    subject: &str,
    unit: &str,
    current: usize,
    position: usize,
    count: usize,
) -> Result<()> {
    if position >= count {
        return Err(EngineError::InvalidInput(format!(
            "{subject}を {} 番目へは移せません（{unit}は全部で {count} 件です）。",
            position.saturating_add(1)
        )));
    }
    if position == current {
        return Err(EngineError::InvalidInput(format!(
            "{subject}はすでに {} 番目です。",
            position + 1
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_moves_forward_and_backward_and_the_others_keep_their_relative_order() {
        let mut items = vec!['a', 'b', 'c', 'd'];
        move_item(&mut items, 3, 0);
        assert_eq!(items, vec!['d', 'a', 'b', 'c']);

        move_item(&mut items, 0, 2);
        assert_eq!(items, vec!['a', 'b', 'd', 'c']);
    }

    #[test]
    fn a_position_outside_the_list_is_refused_with_the_count() {
        let result = ensure_new_position("章", "章", 0, 3, 3);

        let Err(EngineError::InvalidInput(message)) = result else {
            panic!("入力の誤りになるはず: {result:?}");
        };
        assert!(message.contains("4 番目"), "{message}");
        assert!(message.contains("3 件"), "{message}");
    }

    #[test]
    fn the_current_position_is_refused_because_nothing_would_change() {
        let result = ensure_new_position("人物「凛」", "人物", 1, 1, 3);

        let Err(EngineError::InvalidInput(message)) = result else {
            panic!("入力の誤りになるはず: {result:?}");
        };
        assert_eq!(message, "人物「凛」はすでに 2 番目です。");
    }

    #[test]
    fn the_first_and_the_last_position_are_accepted() {
        assert!(ensure_new_position("章", "章", 1, 0, 3).is_ok());
        assert!(ensure_new_position("章", "章", 1, 2, 3).is_ok());
    }
}
