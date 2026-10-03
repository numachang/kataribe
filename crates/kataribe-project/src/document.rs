//! 画面で編集する文書の読み書き。
//!
//! 人物資料（`characters/<id>.md`）と章立て（`plot/chapters/<NN>.md`）は、front matter を項目に分けて
//! 画面へ渡す。それ以外のファイルは文字列のまま扱う。
//!
//! 保存するときは、画面が知らない項目（`extra`）を保存されている側の値で残し、本文だけが変わったときは
//! YAML を書かれたまま（コメントや項目の順番も）残す。

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::frontmatter::{self, YamlError};
use crate::layout::{self, DocumentKind};
use crate::model::{
    Chapter, ChapterId, ChapterMeta, Character, CharacterId, CharacterMeta, ScenePlan,
};
use crate::path::RelPath;
use crate::store::{BackupMode, ContentHash, ProjectStore, TextFile, WriteCondition, WriteOptions};

/// 画面で編集する文書。人物資料と章立ては front matter を項目に分け、それ以外は文字列のまま扱う。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EditableDocument {
    /// ファイルの内容そのまま。
    Text {
        /// ファイル全体の内容。
        content: String,
    },
    /// 人物資料。
    Character {
        /// front matter の項目。
        meta: CharacterMeta,
        /// 本文。
        body: String,
    },
    /// 章立て。
    Chapter {
        /// front matter の項目。
        meta: ChapterMeta,
        /// この章のストーリーライン（本文）。
        body: String,
    },
}

/// 読み込んだ文書。
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFile {
    /// 画面で編集する形にした文書。
    pub document: EditableDocument,
    /// 正規化したファイル全体のハッシュ（[`ProjectStore::read_text`] が返すものと同じ）。
    /// 保存するときの `expected` に渡して、読んだあとの外での変更を検出する。
    pub hash: ContentHash,
    /// 人物資料・章立てなのに front matter を解釈できず、[`EditableDocument::Text`] として返したときの理由。
    /// パスと行番号を含み、利用者にそのまま見せられる。
    pub parse_error: Option<String>,
}

/// ファイルを読み、画面で編集する形にする。
///
/// front matter を解釈できない人物資料・章立ては、直して保存できるよう文字列のまま返す。
pub(crate) fn read_document(
    store: &ProjectStore,
    path: &RelPath,
) -> Result<DocumentFile, ProjectError> {
    let file = store.read_text(path)?;
    let (document, parse_error) = match parse_structured(path, &file.content) {
        Ok(Some(document)) => (document, None),
        Ok(None) => (text_document(file.content), None),
        Err(source) => {
            let reason = ProjectError::Frontmatter {
                path: path.clone(),
                source,
            };
            (text_document(file.content), Some(reason.to_string()))
        }
    };
    Ok(DocumentFile {
        document,
        hash: file.hash,
        parse_error,
    })
}

/// 文書を保存し、保存した内容のハッシュを返す。
///
/// - `expected` が `Some` なら、今のファイルのハッシュがそれと一致するときだけ書く。
/// - `None` なら新規作成だけを許す。
///
/// どちらも合わなければ [`ProjectError::Conflict`] になる。
/// [`EditableDocument::Text`] はどのパスにも書ける（壊れた YAML を文字列のまま直せるように）。
/// 人物資料・章立てはパスの種類が合っていなければ [`ProjectError::DocumentKindMismatch`] になる。
pub(crate) fn write_document(
    store: &ProjectStore,
    path: &RelPath,
    document: &EditableDocument,
    expected: Option<&ContentHash>,
) -> Result<ContentHash, ProjectError> {
    let text = match document {
        EditableDocument::Text { content } => Cow::Borrowed(content.as_str()),
        EditableDocument::Character { meta, body } => {
            let DocumentKind::Character(id) = layout::document_kind(path) else {
                return Err(kind_mismatch(path, "人物資料"));
            };
            let stored = read_stored(store, path, expected)?;
            Cow::Owned(character_text(path, id, meta, body, stored.as_ref())?)
        }
        EditableDocument::Chapter { meta, body } => {
            let DocumentKind::Chapter(id) = layout::document_kind(path) else {
                return Err(kind_mismatch(path, "章立て"));
            };
            let stored = read_stored(store, path, expected)?;
            Cow::Owned(chapter_text(path, id, meta, body, stored.as_ref())?)
        }
    };
    // 条件の確認は write_text が書き込みの排他の中でやり直す。ここで読んだあとに
    // ファイルが変わっていれば、その確認で競合になる。
    store.write_text(
        path,
        &text,
        WriteOptions {
            condition: write_condition(expected),
            backup: BackupMode::Throttled,
        },
    )
}

fn text_document(content: String) -> EditableDocument {
    EditableDocument::Text { content }
}

/// 人物資料・章立てのパスなら項目に分けた文書にする。それ以外のパスは `None`。
fn parse_structured(path: &RelPath, content: &str) -> Result<Option<EditableDocument>, YamlError> {
    match layout::document_kind(path) {
        DocumentKind::Character(id) => {
            let character = Character::parse(id, content)?;
            Ok(Some(EditableDocument::Character {
                meta: character.meta,
                body: character.body,
            }))
        }
        DocumentKind::Chapter(id) => {
            let chapter = Chapter::parse(id, content)?;
            Ok(Some(EditableDocument::Chapter {
                meta: chapter.meta,
                body: chapter.storyline,
            }))
        }
        DocumentKind::Other => Ok(None),
    }
}

fn kind_mismatch(path: &RelPath, expected: &'static str) -> ProjectError {
    ProjectError::DocumentKindMismatch {
        path: path.clone(),
        expected,
    }
}

fn write_condition(expected: Option<&ContentHash>) -> WriteCondition {
    match expected {
        Some(hash) => WriteCondition::Matches(hash.clone()),
        None => WriteCondition::Absent,
    }
}

/// 保存されている今のファイルを読む。呼び出し側の前提（`expected`）と食い違っていれば競合にする。
///
/// 食い違ったファイルの項目を引き継いで書くと、外での変更を取り込んだまま画面の古い内容で
/// 上書きすることになるので、読んだ時点で止める。
fn read_stored(
    store: &ProjectStore,
    path: &RelPath,
    expected: Option<&ContentHash>,
) -> Result<Option<TextFile>, ProjectError> {
    let stored = store.read_text_opt(path)?;
    let matches_expectation = match (&stored, expected) {
        (None, None) => true,
        (Some(file), Some(hash)) => file.hash == *hash,
        (None, Some(_)) | (Some(_), None) => false,
    };
    if matches_expectation {
        Ok(stored)
    } else {
        Err(ProjectError::Conflict { path: path.clone() })
    }
}

/// 人物資料として書き出す内容を決める。
fn character_text(
    path: &RelPath,
    id: CharacterId,
    screen_meta: &CharacterMeta,
    body: &str,
    stored: Option<&TextFile>,
) -> Result<String, ProjectError> {
    let to_project_error = |source| ProjectError::Frontmatter {
        path: path.clone(),
        source,
    };
    let stored_character = stored.and_then(|file| {
        let character = Character::parse(id.clone(), &file.content).ok()?;
        Some((file, character))
    });
    let Some((stored_file, stored_character)) = stored_character else {
        return render_character(id, screen_meta.clone(), body).map_err(to_project_error);
    };

    let mut meta = screen_meta.clone();
    meta.extra = stored_character.meta.extra.clone();
    if meta == stored_character.meta {
        return frontmatter::replace_body(&stored_file.content, body).map_err(to_project_error);
    }
    render_character(id, meta, body).map_err(to_project_error)
}

fn render_character(id: CharacterId, meta: CharacterMeta, body: &str) -> Result<String, YamlError> {
    Character {
        id,
        meta,
        body: body.to_owned(),
    }
    .render()
}

/// 章立てとして書き出す内容を決める。
fn chapter_text(
    path: &RelPath,
    id: ChapterId,
    screen_meta: &ChapterMeta,
    body: &str,
    stored: Option<&TextFile>,
) -> Result<String, ProjectError> {
    let to_project_error = |source| ProjectError::Frontmatter {
        path: path.clone(),
        source,
    };
    let stored_chapter = stored.and_then(|file| {
        let chapter = Chapter::parse(id, &file.content).ok()?;
        Some((file, chapter))
    });
    let Some((stored_file, stored_chapter)) = stored_chapter else {
        return render_chapter(id, screen_meta.clone(), body).map_err(to_project_error);
    };

    let mut meta = screen_meta.clone();
    meta.extra = stored_chapter.meta.extra.clone();
    keep_scene_extras(&mut meta.scenes, &stored_chapter.meta.scenes);
    if meta == stored_chapter.meta {
        return frontmatter::replace_body(&stored_file.content, body).map_err(to_project_error);
    }
    render_chapter(id, meta, body).map_err(to_project_error)
}

fn render_chapter(id: ChapterId, meta: ChapterMeta, body: &str) -> Result<String, YamlError> {
    Chapter {
        id,
        meta,
        storyline: body.to_owned(),
    }
    .render()
}

/// 画面から来たシーンの未知の項目を、保存されている同じ `id` のシーンの値に置き換える。
/// 保存されていない `id` のシーンは、未知の項目なしになる。
fn keep_scene_extras(screen_scenes: &mut [ScenePlan], stored_scenes: &[ScenePlan]) {
    for scene in screen_scenes {
        scene.extra = stored_scenes
            .iter()
            .find(|stored| stored.id == scene.id)
            .map(|stored| stored.extra.clone())
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;
    use crate::model::{FORMAT_VERSION, Manifest, Rating, SceneId};
    use crate::project::Project;

    const CHARACTER_PATH: &str = "characters/kirishima-rin.md";
    const CHAPTER_PATH: &str = "plot/chapters/01.md";

    /// コメント・引用符・ブロック表記・未知の項目（`secret`）を含む人物資料。
    const CHARACTER_TEXT: &str = "---
# 主人公。手で足したコメント
name: 霧島 凛
reading: きりしま りん
role: '主人公'
summary: |
  盲目の少女探偵。
  声の揺れで嘘を聞き分ける。
order: 1
secret: 実は依頼人の妹
---
## 外見
古い本文
";

    /// 未知の項目を、章（`mood`）とシーンごと（`mood`・`weather`）に持つ章立て。
    const CHAPTER_TEXT: &str = "---
# 第一章
title: 雨の匂い
mood: 静か
scenes:
  - id: s01
    title: 事務所に届いた依頼
    summary: 雨の夜、依頼が届く
    mood: 暗い
  - id: s02
    title: 現場へ
    summary: 凛は現場へ向かう
    weather: 雨
---
古いストーリーライン
";

    fn open_project(dir: &TempDir) -> Project {
        let manifest = Manifest {
            format: FORMAT_VERSION,
            title: "テスト作品".to_owned(),
            author: None,
            genre: "general".to_owned(),
            genre_note: None,
            rating: Rating::General,
            target_length: 10_000,
            idea: "静かな夜の物語".to_owned(),
            settings: None,
            extra: BTreeMap::new(),
        };
        Project::create(dir.path(), &manifest).unwrap()
    }

    fn rel(path: &str) -> RelPath {
        RelPath::new(path).unwrap()
    }

    /// 作品フォルダのファイルを、アプリを通さずに直接書く（外部のエディタでの編集を模す）。
    fn put_file(dir: &TempDir, path: &str, content: &str) {
        let full_path = dir.path().join(path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(full_path, content).unwrap();
    }

    fn file_text(dir: &TempDir, path: &str) -> String {
        fs::read_to_string(dir.path().join(path)).unwrap()
    }

    fn read_character(project: &Project) -> (CharacterMeta, String) {
        match project
            .read_document(&rel(CHARACTER_PATH))
            .unwrap()
            .document
        {
            EditableDocument::Character { meta, body } => (meta, body),
            other => panic!("人物資料のはずが {other:?}"),
        }
    }

    fn read_chapter(project: &Project) -> (ChapterMeta, String) {
        match project.read_document(&rel(CHAPTER_PATH)).unwrap().document {
            EditableDocument::Chapter { meta, body } => (meta, body),
            other => panic!("章立てのはずが {other:?}"),
        }
    }

    fn current_hash(project: &Project, path: &str) -> ContentHash {
        project.store().read_text(&rel(path)).unwrap().hash
    }

    fn text_value(value: &str) -> serde_json::Value {
        serde_json::Value::String(value.to_owned())
    }

    fn text_document(content: &str) -> EditableDocument {
        EditableDocument::Text {
            content: content.to_owned(),
        }
    }

    fn empty_character() -> EditableDocument {
        EditableDocument::Character {
            meta: CharacterMeta::default(),
            body: String::new(),
        }
    }

    fn empty_chapter() -> EditableDocument {
        EditableDocument::Chapter {
            meta: ChapterMeta::default(),
            body: String::new(),
        }
    }

    // ---- read_document ----

    #[test]
    fn reads_a_character_file_as_meta_and_body() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);

        let file = project.read_document(&rel(CHARACTER_PATH)).unwrap();

        let EditableDocument::Character { meta, body } = file.document else {
            panic!("人物資料として読めるはず");
        };
        assert_eq!(meta.name, "霧島 凛");
        assert_eq!(meta.reading.as_deref(), Some("きりしま りん"));
        assert_eq!(meta.role, "主人公");
        assert_eq!(
            meta.summary,
            "盲目の少女探偵。\n声の揺れで嘘を聞き分ける。\n"
        );
        assert_eq!(meta.order, Some(1));
        assert_eq!(body, "## 外見\n古い本文\n");
        assert_eq!(file.parse_error, None);
        assert_eq!(file.hash, current_hash(&project, CHARACTER_PATH));
    }

    #[test]
    fn reads_a_chapter_file_with_the_storyline_as_body() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, CHAPTER_TEXT);

        let (meta, body) = read_chapter(&project);

        assert_eq!(meta.title, "雨の匂い");
        let scene_ids: Vec<String> = meta
            .scenes
            .iter()
            .map(|scene| scene.id.to_string())
            .collect();
        assert_eq!(scene_ids, ["s01", "s02"]);
        assert_eq!(body, "古いストーリーライン\n");
    }

    #[test]
    fn reads_other_files_as_plain_text() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let files = [
            ("concept.md", "---\ntitle: 企画\n---\n本文\n"),
            ("manuscript/01/s01.txt", "本文です。\n"),
            ("characters/霧島.md", "id として使えない名前のメモ\n"),
            ("characters/old/rin.md", "サブフォルダの下\n"),
        ];
        for (path, content) in files {
            put_file(&dir, path, content);
        }

        for (path, content) in files {
            let file = project.read_document(&rel(path)).unwrap();
            assert_eq!(file.document, text_document(content), "path: {path}");
            assert_eq!(file.parse_error, None, "path: {path}");
        }
    }

    #[test]
    fn reads_a_character_with_broken_yaml_as_text_with_the_reason() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let broken = "---\nname: [\n---\n本文\n";
        put_file(&dir, CHARACTER_PATH, broken);

        let file = project.read_document(&rel(CHARACTER_PATH)).unwrap();

        assert_eq!(file.document, text_document(broken));
        let reason = file.parse_error.expect("解釈できなかった理由が付くはず");
        assert!(reason.contains(CHARACTER_PATH), "reason: {reason}");
        assert!(reason.contains("行目"), "reason: {reason}");
        assert_eq!(file.hash, current_hash(&project, CHARACTER_PATH));
    }

    #[test]
    fn reads_a_chapter_without_front_matter_as_text_with_the_reason() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, "front matter の無い章\n");

        let file = project.read_document(&rel(CHAPTER_PATH)).unwrap();

        assert_eq!(file.document, text_document("front matter の無い章\n"));
        let reason = file.parse_error.expect("解釈できなかった理由が付くはず");
        assert!(reason.contains(CHAPTER_PATH), "reason: {reason}");
    }

    #[test]
    fn reading_a_missing_file_is_not_found() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let result = project.read_document(&rel(CHARACTER_PATH));

        assert!(matches!(result, Err(ProjectError::NotFound { .. })));
    }

    // ---- write_document: 本文だけを変えるとき ----

    #[test]
    fn changing_only_the_character_body_keeps_the_yaml_as_written() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (meta, _) = read_character(&project);
        let hash = current_hash(&project, CHARACTER_PATH);

        project
            .write_document(
                &rel(CHARACTER_PATH),
                &EditableDocument::Character {
                    meta,
                    body: "## 外見\n新しい本文\n".to_owned(),
                },
                Some(&hash),
            )
            .unwrap();

        let expected = CHARACTER_TEXT.replace("古い本文", "新しい本文");
        assert_eq!(file_text(&dir, CHARACTER_PATH), expected);
    }

    #[test]
    fn changing_only_the_chapter_body_keeps_the_yaml_as_written() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, CHAPTER_TEXT);
        let (meta, _) = read_chapter(&project);
        let hash = current_hash(&project, CHAPTER_PATH);

        project
            .write_document(
                &rel(CHAPTER_PATH),
                &EditableDocument::Chapter {
                    meta,
                    body: "新しいストーリーライン\n".to_owned(),
                },
                Some(&hash),
            )
            .unwrap();

        let expected = CHAPTER_TEXT.replace("古いストーリーライン", "新しいストーリーライン");
        assert_eq!(file_text(&dir, CHAPTER_PATH), expected);
    }

    #[test]
    fn a_body_only_change_keeps_the_yaml_even_when_the_screen_sends_other_extra_values() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (mut meta, _) = read_character(&project);
        meta.extra.clear();
        meta.extra
            .insert("secret".to_owned(), text_value("画面が知らない別の値"));
        let hash = current_hash(&project, CHARACTER_PATH);

        project
            .write_document(
                &rel(CHARACTER_PATH),
                &EditableDocument::Character {
                    meta,
                    body: "## 外見\n新しい本文\n".to_owned(),
                },
                Some(&hash),
            )
            .unwrap();

        let expected = CHARACTER_TEXT.replace("古い本文", "新しい本文");
        assert_eq!(file_text(&dir, CHARACTER_PATH), expected);
    }

    #[test]
    fn saving_the_unchanged_document_does_not_touch_the_file() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, CHAPTER_TEXT);
        let file = project.read_document(&rel(CHAPTER_PATH)).unwrap();

        let hash = project
            .write_document(&rel(CHAPTER_PATH), &file.document, Some(&file.hash))
            .unwrap();

        assert_eq!(hash, file.hash);
        assert_eq!(file_text(&dir, CHAPTER_PATH), CHAPTER_TEXT);
    }

    // ---- write_document: 項目を変えるとき ----

    #[test]
    fn changing_character_meta_rewrites_the_yaml_and_keeps_unknown_fields() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (mut meta, body) = read_character(&project);
        meta.role = "探偵".to_owned();
        // 画面から来た extra に別の値が入っていても、保存されている側の値が使われる
        meta.extra.clear();
        meta.extra
            .insert("secret".to_owned(), text_value("画面側の値"));
        meta.extra
            .insert("screen_only".to_owned(), text_value("画面だけの項目"));
        let hash = current_hash(&project, CHARACTER_PATH);

        project
            .write_document(
                &rel(CHARACTER_PATH),
                &EditableDocument::Character {
                    meta,
                    body: body.clone(),
                },
                Some(&hash),
            )
            .unwrap();

        let (saved, saved_body) = read_character(&project);
        assert_eq!(saved.role, "探偵");
        assert_eq!(saved.name, "霧島 凛");
        assert_eq!(saved.extra.len(), 1);
        assert_eq!(saved.extra["secret"], text_value("実は依頼人の妹"));
        assert_eq!(saved_body, body);
    }

    #[test]
    fn changing_chapter_meta_keeps_unknown_fields_at_top_level_and_per_scene() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, CHAPTER_TEXT);
        let (mut meta, body) = read_chapter(&project);
        meta.title = "雨の匂い（改）".to_owned();
        meta.scenes[0].summary = "書き換えた要約".to_owned();
        meta.extra.clear();
        meta.extra
            .insert("mood".to_owned(), text_value("画面側の値"));
        meta.scenes[1].extra.clear();
        meta.scenes[1]
            .extra
            .insert("weather".to_owned(), text_value("画面側の値"));
        let hash = current_hash(&project, CHAPTER_PATH);

        project
            .write_document(
                &rel(CHAPTER_PATH),
                &EditableDocument::Chapter { meta, body },
                Some(&hash),
            )
            .unwrap();

        let (saved, _) = read_chapter(&project);
        assert_eq!(saved.title, "雨の匂い（改）");
        assert_eq!(saved.scenes[0].summary, "書き換えた要約");
        assert_eq!(saved.extra["mood"], text_value("静か"));
        assert_eq!(saved.scenes[0].extra["mood"], text_value("暗い"));
        assert_eq!(saved.scenes[1].extra["weather"], text_value("雨"));
    }

    #[test]
    fn scene_extra_fields_follow_the_scene_id_when_scenes_are_reordered_or_added() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHAPTER_PATH, CHAPTER_TEXT);
        let (mut meta, body) = read_chapter(&project);
        meta.scenes.reverse();
        let mut added = meta.scenes[0].clone();
        added.id = SceneId::new("s03").unwrap();
        added
            .extra
            .insert("weather".to_owned(), text_value("画面側の値"));
        meta.scenes.push(added);
        let hash = current_hash(&project, CHAPTER_PATH);

        project
            .write_document(
                &rel(CHAPTER_PATH),
                &EditableDocument::Chapter { meta, body },
                Some(&hash),
            )
            .unwrap();

        let (saved, _) = read_chapter(&project);
        let ids: Vec<String> = saved
            .scenes
            .iter()
            .map(|scene| scene.id.to_string())
            .collect();
        assert_eq!(ids, ["s02", "s01", "s03"]);
        assert_eq!(saved.scenes[0].extra["weather"], text_value("雨"));
        assert_eq!(saved.scenes[1].extra["mood"], text_value("暗い"));
        assert!(saved.scenes[2].extra.is_empty());
    }

    // ---- write_document: 競合・新規作成 ----

    #[test]
    fn a_stale_expected_hash_is_a_conflict_and_leaves_the_file_unchanged() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (meta, body) = read_character(&project);
        let stale_hash = current_hash(&project, CHARACTER_PATH);
        let edited_outside = CHARACTER_TEXT.replace("古い本文", "外で直した本文");
        put_file(&dir, CHARACTER_PATH, &edited_outside);

        let result = project.write_document(
            &rel(CHARACTER_PATH),
            &EditableDocument::Character { meta, body },
            Some(&stale_hash),
        );

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(file_text(&dir, CHARACTER_PATH), edited_outside);
    }

    #[test]
    fn a_stale_expected_hash_is_a_conflict_for_text_too() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, "concept.md", "初回\n");
        let stale_hash = current_hash(&project, "concept.md");
        put_file(&dir, "concept.md", "外で変えた\n");

        let result = project.write_document(
            &rel("concept.md"),
            &text_document("画面の内容\n"),
            Some(&stale_hash),
        );

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(file_text(&dir, "concept.md"), "外で変えた\n");
    }

    #[test]
    fn creating_over_an_existing_file_is_a_conflict() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);

        let result = project.write_document(&rel(CHARACTER_PATH), &empty_character(), None);

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(file_text(&dir, CHARACTER_PATH), CHARACTER_TEXT);
    }

    #[test]
    fn a_missing_file_with_an_expected_hash_is_a_conflict() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let hash = project
            .write_document(&rel("concept.md"), &text_document("別のファイル\n"), None)
            .unwrap();

        let result = project.write_document(&rel(CHARACTER_PATH), &empty_character(), Some(&hash));

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert!(!dir.path().join(CHARACTER_PATH).exists());
    }

    #[test]
    fn creates_a_new_character_file_from_the_screen_document() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let meta = CharacterMeta {
            name: "佐藤 健二".to_owned(),
            role: "依頼人".to_owned(),
            ..CharacterMeta::default()
        };

        project
            .write_document(
                &rel("characters/sato-kenji.md"),
                &EditableDocument::Character {
                    meta: meta.clone(),
                    body: "## 外見\n".to_owned(),
                },
                None,
            )
            .unwrap();

        let character = project
            .character(&CharacterId::new("sato-kenji").unwrap())
            .unwrap()
            .expect("作られているはず");
        assert_eq!(character.meta, meta);
        assert_eq!(character.body, "## 外見\n");
    }

    #[test]
    fn creates_a_new_chapter_file_with_its_scenes() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let scene = ScenePlan {
            id: SceneId::from_number(1),
            title: "事務所".to_owned(),
            summary: "依頼が届く".to_owned(),
            pov: None,
            characters: vec!["霧島 凛".to_owned()],
            place: None,
            time: None,
            target_chars: Some(2000),
            beats: Vec::new(),
            extra: BTreeMap::new(),
        };
        let meta = ChapterMeta {
            title: "雨の匂い".to_owned(),
            scenes: vec![scene],
            extra: BTreeMap::new(),
        };

        project
            .write_document(
                &rel(CHAPTER_PATH),
                &EditableDocument::Chapter {
                    meta: meta.clone(),
                    body: "ストーリーライン\n".to_owned(),
                },
                None,
            )
            .unwrap();

        let (saved, body) = read_chapter(&project);
        assert_eq!(saved, meta);
        assert_eq!(body, "ストーリーライン\n");
    }

    // ---- write_document: 種類とパス ----

    #[test]
    fn a_character_document_cannot_be_saved_to_a_chapter_path() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let result = project.write_document(&rel(CHAPTER_PATH), &empty_character(), None);

        assert!(matches!(
            result,
            Err(ProjectError::DocumentKindMismatch { .. })
        ));
        assert!(!dir.path().join(CHAPTER_PATH).exists());
    }

    #[test]
    fn a_structured_document_cannot_be_saved_to_a_path_of_another_kind() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        let cases = [
            (CHARACTER_PATH, empty_chapter()),
            ("concept.md", empty_chapter()),
            ("plot/chapters/draft/01.md", empty_chapter()),
            ("concept.md", empty_character()),
            ("characters/霧島.md", empty_character()),
        ];

        for (path, document) in cases {
            let result = project.write_document(&rel(path), &document, None);

            assert!(
                matches!(result, Err(ProjectError::DocumentKindMismatch { .. })),
                "path: {path}"
            );
            assert!(!dir.path().join(path).exists(), "path: {path}");
        }
    }

    #[test]
    fn text_can_be_saved_to_a_character_path_even_when_the_yaml_is_broken() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, "---\nname: [\n---\n本文\n");
        let hash = current_hash(&project, CHARACTER_PATH);

        project
            .write_document(
                &rel(CHARACTER_PATH),
                &text_document(CHARACTER_TEXT),
                Some(&hash),
            )
            .unwrap();

        assert_eq!(file_text(&dir, CHARACTER_PATH), CHARACTER_TEXT);
        let file = project.read_document(&rel(CHARACTER_PATH)).unwrap();
        assert!(matches!(file.document, EditableDocument::Character { .. }));
        assert_eq!(file.parse_error, None);
    }

    #[test]
    fn a_structured_document_overwrites_an_unparsable_file_using_the_screen_meta() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (meta, body) = read_character(&project);
        // 画面で開いたあとに、外で YAML が壊された。利用者が競合を承知で「上書き」した
        put_file(&dir, CHARACTER_PATH, "---\nname: [\n---\n壊れた\n");
        let broken_hash = current_hash(&project, CHARACTER_PATH);

        project
            .write_document(
                &rel(CHARACTER_PATH),
                &EditableDocument::Character {
                    meta: meta.clone(),
                    body: body.clone(),
                },
                Some(&broken_hash),
            )
            .unwrap();

        // 保存されている側に引き継げる項目が無いので、画面から来た meta がそのまま使われる
        let (saved, saved_body) = read_character(&project);
        assert_eq!(saved, meta);
        assert_eq!(saved_body, body);
    }

    // ---- write_document: ハッシュ ----

    #[test]
    fn the_returned_hash_matches_the_hash_of_the_next_read() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);
        put_file(&dir, CHARACTER_PATH, CHARACTER_TEXT);
        let (mut meta, body) = read_character(&project);
        meta.role = "探偵".to_owned();
        let documents = [
            EditableDocument::Character { meta, body },
            text_document(CHARACTER_TEXT),
        ];
        let mut expected = current_hash(&project, CHARACTER_PATH);

        for document in documents {
            let returned = project
                .write_document(&rel(CHARACTER_PATH), &document, Some(&expected))
                .unwrap();

            let read = project.read_document(&rel(CHARACTER_PATH)).unwrap();
            assert_eq!(read.hash, returned);
            expected = returned;
        }
    }

    #[test]
    fn the_returned_hash_matches_the_next_read_for_text_with_crlf() {
        let dir = TempDir::new().unwrap();
        let project = open_project(&dir);

        let returned = project
            .write_document(
                &rel("concept.md"),
                &text_document("一行目\r\n二行目\r\n"),
                None,
            )
            .unwrap();

        let read = project.read_document(&rel("concept.md")).unwrap();
        assert_eq!(read.hash, returned);
        assert_eq!(file_text(&dir, "concept.md"), "一行目\n二行目\n");
    }

    #[test]
    fn editable_document_is_tagged_with_kind_in_json() {
        let value = serde_json::to_value(text_document("本文")).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "kind": "text", "content": "本文" })
        );

        let parsed: EditableDocument = serde_json::from_value(serde_json::json!({
            "kind": "character",
            "meta": { "name": "霧島 凛", "role": "", "summary": "" },
            "body": "本文"
        }))
        .unwrap();

        assert!(matches!(parsed, EditableDocument::Character { .. }));
    }
}
