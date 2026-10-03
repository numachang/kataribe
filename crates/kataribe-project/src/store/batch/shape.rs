//! 反映する変更の形の検証。何かを確かめたり動かしたりする前に行う。
//!
//! 画面から戻ってくる値なので、ここで必ず検証する。パスの重なりは、同じパス（大文字小文字の違いは同じとみなす）だけでなく、
//! フォルダとその中のパスの重なりも見る。どの組み合わせが許されるかは [`allows_overlap`] の表にまとめてある。

use super::PendingChange;
use super::namespace::{Relation, key_of, relation_between};
use crate::error::ProjectError;
use crate::layout;
use crate::path::RelPath;

/// 変更の形を確かめる。誤りがあれば [`ProjectError::InvalidChangeSet`]。
pub(super) fn validate(changes: &[PendingChange<'_>]) -> Result<(), ProjectError> {
    let touches = touches_of(changes)?;
    for (index, first) in touches.iter().enumerate() {
        for second in &touches[index + 1..] {
            check_pair(first, second)?;
        }
    }
    Ok(())
}

/// 変更が触れるパスの役割。順序は [`check_pair`] が組を並べ替えるのに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Expect,
    Trash,
    MoveFrom,
    MoveTo,
    Write,
}

/// 変更が触れる 1 つのパス。
struct Touch<'a> {
    role: Role,
    path: &'a RelPath,
    key: String,
    /// 移動の役割のとき、その移動の通し番号（同じ移動の元と先を見分けるため）。
    move_index: Option<usize>,
}

fn touches_of<'a>(changes: &[PendingChange<'a>]) -> Result<Vec<Touch<'a>>, ProjectError> {
    let mut touches = Vec::new();
    let mut move_count = 0;
    for change in changes {
        match change {
            PendingChange::Write(write) => touches.push(touch(Role::Write, write.path, None)),
            PendingChange::Expect { path, .. } => touches.push(touch(Role::Expect, path, None)),
            PendingChange::Trash { path, .. } => {
                ensure_can_be_changed(path, "ゴミ箱へ移せません")?;
                touches.push(touch(Role::Trash, path, None));
            }
            PendingChange::Move { from, to } => {
                ensure_can_be_changed(from, "移動できません")?;
                ensure_can_be_changed(to, "移動先にできません")?;
                ensure_not_into_itself(from, to)?;
                touches.push(touch(Role::MoveFrom, from, Some(move_count)));
                touches.push(touch(Role::MoveTo, to, Some(move_count)));
                move_count += 1;
            }
        }
    }
    Ok(touches)
}

fn touch(role: Role, path: &RelPath, move_index: Option<usize>) -> Touch<'_> {
    Touch {
        role,
        path,
        key: key_of(path),
        move_index,
    }
}

/// 作品情報と、アプリの内部データ（`.kataribe/`）は、ゴミ箱へ移したり動かしたりできない。
fn ensure_can_be_changed(path: &RelPath, refusal: &str) -> Result<(), ProjectError> {
    let top_level = path.as_str().split('/').next().unwrap_or_default();
    let is_protected = top_level.eq_ignore_ascii_case(layout::INTERNAL_DIR)
        || path.as_str().eq_ignore_ascii_case(layout::MANIFEST);
    if is_protected {
        return Err(invalid(format!(
            "{path} は{refusal}（作品情報と、アプリの内部データ（.kataribe/）は対象外です）。"
        )));
    }
    Ok(())
}

/// 同じ場所や、自分の中（フォルダを自分の下）へは移せない。
fn ensure_not_into_itself(from: &RelPath, to: &RelPath) -> Result<(), ProjectError> {
    if relation_between(&key_of(from), &key_of(to)) != Relation::Disjoint {
        return Err(invalid(format!(
            "{from} を {to} へは移せません（同じ場所、または自分の中になります）。"
        )));
    }
    Ok(())
}

fn check_pair(first: &Touch<'_>, second: &Touch<'_>) -> Result<(), ProjectError> {
    // 役割の順に並べ替えて、表を 1 通りだけ書けば済むようにする
    let (earlier, later, relation) = if first.role <= second.role {
        (first, second, relation_between(&first.key, &second.key))
    } else {
        (second, first, relation_between(&second.key, &first.key))
    };
    let is_same_move = earlier.move_index.is_some() && earlier.move_index == later.move_index;
    if relation == Relation::Disjoint
        || is_same_move
        || allows_overlap(earlier.role, later.role, relation)
    {
        return Ok(());
    }
    if relation == Relation::Equal {
        return Err(invalid(format!(
            "{} への変更が重なっています。",
            earlier.path
        )));
    }
    Err(invalid(format!(
        "{} と {} への変更が重なっています。",
        earlier.path, later.path
    )))
}

/// 2 つの変更のパスが重なっていても許されるか。`relation` は `earlier` から見た `later` の位置関係
/// （`Below` は earlier が later の下、`Above` は earlier が later の上）。重なっていない組は呼ばれない。
///
/// | earlier \ later | 状態の確認 | ゴミ箱 | 移動元 | 移動先 | 書き込み |
/// |---|---|---|---|---|---|
/// | 状態の確認 | 入れ子は可 | 不可 | 可 | 可 | 不可 |
/// | ゴミ箱 | | 不可 | 不可 | 同じパスだけ可 | 不可 |
/// | 移動元 | | | 不可 | 同じパスだけ可 | 同じパス・書き込みが下なら可 |
/// | 移動先 | | | | 不可 | 同じパス・書き込みが下なら可 |
/// | 書き込み | | | | | 不可 |
///
/// 移動元と移動先が同じパスになるのは、入れ替えや番号をずらす移動のつなぎ。書き込みが移動先と重なるのは、
/// 移した先のファイルを書き換える場合、移動元と重なるのは、空いた場所に新しく書く場合。
fn allows_overlap(earlier: Role, later: Role, relation: Relation) -> bool {
    let is_equal = relation == Relation::Equal;
    match (earlier, later) {
        (Role::Expect, Role::Expect) => !is_equal,
        (Role::Expect, Role::MoveFrom | Role::MoveTo) => true,
        (Role::Trash | Role::MoveFrom, Role::MoveTo) => is_equal,
        (Role::MoveFrom | Role::MoveTo, Role::Write) => is_equal || relation == Relation::Above,
        _ => false,
    }
}

fn invalid(reason: String) -> ProjectError {
    ProjectError::InvalidChangeSet { reason }
}

#[cfg(test)]
mod tests {
    use super::super::PendingWrite;
    use super::super::{EntryCondition, WriteCondition};
    use super::*;
    use crate::store::ContentHash;

    fn rel(path: &str) -> RelPath {
        RelPath::new(path).unwrap()
    }

    fn hash() -> ContentHash {
        ContentHash::of(b"x")
    }

    fn write(path: &RelPath) -> PendingChange<'_> {
        PendingChange::Write(PendingWrite {
            path,
            content: "x",
            condition: WriteCondition::Any,
        })
    }

    fn trash_file(path: &RelPath) -> PendingChange<'_> {
        PendingChange::Trash {
            path,
            expected: EntryCondition::File(hash()),
        }
    }

    fn trash_folder(path: &RelPath) -> PendingChange<'_> {
        PendingChange::Trash {
            path,
            expected: EntryCondition::Folder(Vec::new()),
        }
    }

    fn expect_absent(path: &RelPath) -> PendingChange<'_> {
        PendingChange::Expect {
            path,
            expected: None,
        }
    }

    fn move_to<'a>(from: &'a RelPath, to: &'a RelPath) -> PendingChange<'a> {
        PendingChange::Move { from, to }
    }

    fn is_invalid(changes: &[PendingChange<'_>]) -> bool {
        matches!(
            validate(changes),
            Err(ProjectError::InvalidChangeSet { .. })
        )
    }

    #[test]
    fn independent_changes_are_valid() {
        let (a, b, c, d) = (
            rel("concept.md"),
            rel("style.md"),
            rel("manuscript/03"),
            rel("manuscript/04"),
        );

        assert!(!is_invalid(&[write(&a), trash_file(&b), move_to(&c, &d)]));
    }

    #[test]
    fn moves_that_chain_or_swap_through_the_same_path_are_valid() {
        let (one, two, three) = (
            rel("manuscript/01"),
            rel("manuscript/02"),
            rel("manuscript/03"),
        );

        assert!(!is_invalid(&[move_to(&two, &three), move_to(&one, &two)]));
        assert!(!is_invalid(&[move_to(&one, &two), move_to(&two, &one)]));
    }

    #[test]
    fn writing_into_the_place_a_move_empties_or_fills_is_valid() {
        let (from, to) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
        let (folder_from, folder_to) = (rel("manuscript/03"), rel("manuscript/04"));
        let inside_destination = rel("manuscript/04/s05.txt");

        assert!(!is_invalid(&[move_to(&from, &to), write(&from)]));
        assert!(!is_invalid(&[move_to(&from, &to), write(&to)]));
        assert!(!is_invalid(&[
            move_to(&folder_from, &folder_to),
            write(&inside_destination)
        ]));
    }

    #[test]
    fn moving_into_a_place_that_a_trash_empties_is_valid() {
        let (chapter, next) = (rel("plot/chapters/02.md"), rel("plot/chapters/03.md"));

        assert!(!is_invalid(&[
            trash_file(&chapter),
            move_to(&next, &chapter)
        ]));
    }

    #[test]
    fn expecting_the_state_of_a_path_that_a_move_touches_is_valid() {
        let (from, to) = (rel("manuscript/02"), rel("manuscript/03"));

        assert!(!is_invalid(&[expect_absent(&to), move_to(&from, &to)]));
    }

    #[test]
    fn trash_and_move_are_refused_for_the_manifest_and_internal_data() {
        let ordinary = rel("manuscript/03");
        for protected in [
            "kataribe.yaml",
            "Kataribe.yaml",
            ".kataribe/cache/summary.json",
            ".Kataribe/trash",
        ] {
            let path = rel(protected);
            assert!(is_invalid(&[trash_file(&path)]), "{protected}");
            assert!(is_invalid(&[move_to(&path, &ordinary)]), "{protected}");
            assert!(is_invalid(&[move_to(&ordinary, &path)]), "{protected}");
        }
    }

    #[test]
    fn a_folder_cannot_be_moved_into_itself_or_onto_itself() {
        let (folder, inside) = (rel("manuscript/03"), rel("manuscript/03/old"));
        let other_case = rel("Manuscript/03");

        assert!(is_invalid(&[move_to(&folder, &inside)]));
        assert!(is_invalid(&[move_to(&inside, &folder)]));
        assert!(is_invalid(&[move_to(&folder, &folder)]));
        assert!(is_invalid(&[move_to(&folder, &other_case)]));
    }

    #[test]
    fn two_changes_to_the_same_path_overlap_whatever_their_kind() {
        let path = rel("characters/rin.md");
        let other_case = rel("Characters/Rin.md");
        let elsewhere = rel("manuscript/04");

        assert!(is_invalid(&[write(&path), write(&path)]));
        assert!(is_invalid(&[trash_file(&path), write(&path)]));
        assert!(is_invalid(&[trash_file(&path), write(&other_case)]));
        assert!(is_invalid(&[trash_file(&path), trash_file(&path)]));
        assert!(is_invalid(&[expect_absent(&path), write(&path)]));
        assert!(is_invalid(&[expect_absent(&path), expect_absent(&path)]));
        assert!(is_invalid(&[move_to(&path, &elsewhere), trash_file(&path)]));
    }

    #[test]
    fn writing_under_a_trashed_folder_overlaps() {
        let (folder, inside) = (rel("manuscript/03"), rel("manuscript/03/s09.txt"));

        assert!(is_invalid(&[trash_folder(&folder), write(&inside)]));
        assert!(is_invalid(&[trash_folder(&inside), write(&folder)]));
    }

    #[test]
    fn two_moves_cannot_share_a_source_or_a_destination_even_through_a_folder() {
        let (a, b, c, folder, inside) = (
            rel("manuscript/01"),
            rel("manuscript/02"),
            rel("manuscript/03"),
            rel("manuscript/04"),
            rel("manuscript/04/x"),
        );

        assert!(is_invalid(&[move_to(&a, &c), move_to(&a, &b)]));
        assert!(is_invalid(&[move_to(&a, &c), move_to(&b, &c)]));
        assert!(is_invalid(&[move_to(&a, &folder), move_to(&b, &inside)]));
        assert!(is_invalid(&[move_to(&folder, &c), move_to(&inside, &b)]));
    }

    #[test]
    fn a_move_destination_cannot_be_inside_a_trashed_or_moved_folder() {
        let (folder, inside, other) = (
            rel("manuscript/03"),
            rel("manuscript/03/x.txt"),
            rel("plot/x.md"),
        );
        let away = rel("manuscript/09");

        assert!(is_invalid(&[
            trash_folder(&folder),
            move_to(&other, &inside)
        ]));
        assert!(is_invalid(&[
            move_to(&folder, &away),
            move_to(&other, &inside)
        ]));
    }

    #[test]
    fn writing_above_a_move_destination_is_refused() {
        let (from, folder, above) = (rel("plot/x.md"), rel("manuscript/04"), rel("manuscript"));

        assert!(is_invalid(&[move_to(&from, &folder), write(&above)]));
    }
}
