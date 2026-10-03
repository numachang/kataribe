//! 変更を反映したあとの、パスの対応（どのパスの中身が、今どこにあるものか）。
//!
//! 章を途中に足すときの「03 を 04 へ移し、空いた 03 に新しい章を書く」のように、書き込みの条件は
//! 移動した後の状態に対して確かめる。そのために、移動・ゴミ箱で動くパスを覚えておく。
//! Windows は大文字小文字を区別しないので、比べるときは小文字にそろえる。

use super::PendingChange;
use crate::error::ProjectError;
use crate::path::RelPath;

/// 2 つのパスの位置関係。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Relation {
    /// 重ならない。
    Disjoint,
    /// 同じパス。
    Equal,
    /// 1 つ目が、2 つ目のフォルダの下にある。
    Below,
    /// 1 つ目が、2 つ目を中に含むフォルダ（2 つ目が 1 つ目の下にある）。
    Above,
}

/// 大文字小文字を無視して比べるための鍵。
pub(super) fn key_of(path: &RelPath) -> String {
    path.as_str().to_lowercase()
}

/// 鍵どうしの位置関係。
pub(super) fn relation_between(first: &str, second: &str) -> Relation {
    if first == second {
        Relation::Equal
    } else if is_below(first, second) {
        Relation::Below
    } else if is_below(second, first) {
        Relation::Above
    } else {
        Relation::Disjoint
    }
}

/// `path` が `dir` の下（中のどこか）にあるか。
fn is_below(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// 移動で動くパスと、動いて空くパス。
#[derive(Debug, Default)]
pub(super) struct Namespace {
    moves: Vec<MoveEntry>,
    /// ゴミ箱へ移すパスと、移動元のパスの鍵。ここから中身が無くなる。
    vacated: Vec<String>,
}

#[derive(Debug)]
struct MoveEntry {
    from: RelPath,
    to_key: String,
}

impl Namespace {
    /// `changes` の移動とゴミ箱から作る。
    pub(super) fn new(changes: &[PendingChange<'_>]) -> Self {
        let mut namespace = Self::default();
        for change in changes {
            match change {
                PendingChange::Move { from, to } => {
                    namespace.moves.push(MoveEntry {
                        from: (*from).clone(),
                        to_key: key_of(to),
                    });
                    namespace.vacated.push(key_of(from));
                }
                PendingChange::Trash { path, .. } => namespace.vacated.push(key_of(path)),
                PendingChange::Write(_) | PendingChange::Expect { .. } => {}
            }
        }
        namespace
    }

    /// `path` が、移動やゴミ箱で空く場所（そのもの、またはその下）か。
    pub(super) fn is_vacated(&self, path: &RelPath) -> bool {
        let key = key_of(path);
        self.vacated.iter().any(|vacated| {
            matches!(
                relation_between(&key, vacated),
                Relation::Equal | Relation::Below
            )
        })
    }

    /// 変更を反映したあとに `path` にあるものが、今どこにあるか。
    ///
    /// 移動の行き先（の下）ならその移動元の対応するパス、移動やゴミ箱で空く場所なら `None`
    /// （書き込みの条件では「無いこと」として扱う）、そうでなければ `path` そのもの。
    pub(super) fn source_of(&self, path: &RelPath) -> Result<Option<RelPath>, ProjectError> {
        let key = key_of(path);
        for entry in &self.moves {
            match relation_between(&key, &entry.to_key) {
                Relation::Equal => return Ok(Some(entry.from.clone())),
                Relation::Below => {
                    let depth = entry.to_key.split('/').count();
                    let rest: Vec<&str> = path.as_str().split('/').skip(depth).collect();
                    let source = RelPath::new(&format!("{}/{}", entry.from, rest.join("/")))?;
                    return Ok(Some(source));
                }
                Relation::Disjoint | Relation::Above => {}
            }
        }
        if self.is_vacated(path) {
            return Ok(None);
        }
        Ok(Some(path.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(path: &str) -> RelPath {
        RelPath::new(path).unwrap()
    }

    #[test]
    fn relations_distinguish_equal_below_above_and_disjoint_paths_ignoring_case() {
        assert_eq!(relation_between("a/b", "a/b"), Relation::Equal);
        assert_eq!(relation_between("a/b/c", "a/b"), Relation::Below);
        assert_eq!(relation_between("a/b", "a/b/c"), Relation::Above);
        assert_eq!(relation_between("a/bc", "a/b"), Relation::Disjoint);
        assert_eq!(relation_between("a/b", "a/c"), Relation::Disjoint);
        assert_eq!(key_of(&rel("Manuscript/03")), "manuscript/03");
    }

    #[test]
    fn a_moved_destination_and_the_paths_below_it_come_from_the_source() {
        let (from, to) = (rel("manuscript/03"), rel("manuscript/04"));
        let changes = [PendingChange::Move {
            from: &from,
            to: &to,
        }];
        let namespace = Namespace::new(&changes);

        assert_eq!(
            namespace.source_of(&rel("manuscript/04")).unwrap(),
            Some(rel("manuscript/03"))
        );
        assert_eq!(
            namespace.source_of(&rel("Manuscript/04/s01.txt")).unwrap(),
            Some(rel("manuscript/03/s01.txt"))
        );
    }

    #[test]
    fn a_place_emptied_by_a_move_has_no_source_and_other_paths_are_unchanged() {
        let (from, to) = (rel("plot/chapters/03.md"), rel("plot/chapters/04.md"));
        let changes = [PendingChange::Move {
            from: &from,
            to: &to,
        }];
        let namespace = Namespace::new(&changes);

        assert_eq!(
            namespace.source_of(&rel("plot/chapters/03.md")).unwrap(),
            None
        );
        assert_eq!(
            namespace.source_of(&rel("plot/chapters/01.md")).unwrap(),
            Some(rel("plot/chapters/01.md"))
        );
        assert!(namespace.is_vacated(&rel("plot/chapters/03.md")));
        assert!(!namespace.is_vacated(&rel("plot/chapters/04.md")));
    }

    #[test]
    fn swapped_paths_each_come_from_the_other() {
        let (first, second) = (rel("plot/chapters/01.md"), rel("plot/chapters/02.md"));
        let changes = [
            PendingChange::Move {
                from: &first,
                to: &second,
            },
            PendingChange::Move {
                from: &second,
                to: &first,
            },
        ];
        let namespace = Namespace::new(&changes);

        assert_eq!(namespace.source_of(&first).unwrap(), Some(second.clone()));
        assert_eq!(namespace.source_of(&second).unwrap(), Some(first));
    }
}
