//! 作品の作成・読み込みと、型付きの取得。
//!
//! [`Project`] はキャッシュを持たず、毎回ファイルから読む。外部エディタや
//! `git checkout` による変更を常に反映するため。

use std::fs;
use std::path::Path;

use crate::document::{self, DocumentFile, EditableDocument};
use crate::error::ProjectError;
use crate::layout::{self, DocumentKind};
use crate::model::{
    Chapter, ChapterId, Character, CharacterId, FORMAT_VERSION, Manifest, MarkdownDoc, SceneId,
};
use crate::path::RelPath;
use crate::store::{
    BackupMode, ContentHash, DirEntryKind, ProjectStore, WriteCondition, WriteOptions,
};

/// 作品フォルダそのもの。
#[derive(Debug)]
pub struct Project {
    store: ProjectStore,
}

impl Project {
    /// `dir` に新しい作品フォルダを作る。
    ///
    /// `dir` は存在しないか、空でなければならない（`.git` だけがある場合も空と見なす）。
    pub fn create(dir: &Path, manifest: &Manifest) -> Result<Self, ProjectError> {
        ensure_directory_is_creatable(dir)?;
        fs::create_dir_all(dir).map_err(|source| ProjectError::RootNotOpenable {
            root: dir.to_path_buf(),
            source,
        })?;

        let store = ProjectStore::open(dir)?;
        let manifest_path = RelPath::new(layout::MANIFEST)?;
        let manifest_text = manifest
            .render()
            .map_err(|source| ProjectError::Frontmatter {
                path: manifest_path.clone(),
                source,
            })?;
        let created_only = WriteOptions {
            condition: WriteCondition::Absent,
            backup: BackupMode::Never,
        };
        store.write_text(&manifest_path, &manifest_text, created_only.clone())?;
        store.write_text(
            &RelPath::new(layout::GITIGNORE)?,
            layout::GITIGNORE_CONTENT,
            created_only.clone(),
        )?;
        store.write_text(
            &RelPath::new(layout::GITATTRIBUTES)?,
            layout::GITATTRIBUTES_CONTENT,
            created_only,
        )?;

        for dir_name in [
            layout::WORLD_DIR,
            layout::CHARACTERS_DIR,
            layout::CHAPTERS_DIR,
            layout::MANUSCRIPT_DIR,
        ] {
            store.ensure_dir(&RelPath::new(dir_name)?)?;
        }

        Self::open(dir)
    }

    /// 既存の作品フォルダを開く。
    pub fn open(dir: &Path) -> Result<Self, ProjectError> {
        let store = ProjectStore::open(dir)?;
        let manifest_path = RelPath::new(layout::MANIFEST)?;
        let Some(text_file) = store.read_text_opt(&manifest_path)? else {
            return Err(ProjectError::NotAProject {
                root: dir.to_path_buf(),
            });
        };
        let manifest =
            Manifest::parse(&text_file.content).map_err(|source| ProjectError::Frontmatter {
                path: manifest_path,
                source,
            })?;
        if manifest.format > FORMAT_VERSION {
            return Err(ProjectError::UnsupportedFormat {
                found: manifest.format,
                supported: FORMAT_VERSION,
            });
        }
        Ok(Self { store })
    }

    /// 作品フォルダのルート。
    #[must_use]
    pub fn root(&self) -> &Path {
        self.store.root()
    }

    /// この作品を読み書きする [`ProjectStore`]。
    #[must_use]
    pub fn store(&self) -> &ProjectStore {
        &self.store
    }

    /// `kataribe.yaml` を読み込む。
    pub fn manifest(&self) -> Result<Manifest, ProjectError> {
        Ok(self.manifest_with_hash()?.0)
    }

    /// `kataribe.yaml` を、読み込んだ時点の内容のハッシュと一緒に読み込む。
    /// ハッシュは [`Self::update_manifest`] の `expected` に渡し、その間の外での変更を検出するのに使う。
    pub fn manifest_with_hash(&self) -> Result<(Manifest, ContentHash), ProjectError> {
        let path = RelPath::new(layout::MANIFEST)?;
        let text_file = self.store.read_text(&path)?;
        let manifest = Manifest::parse(&text_file.content)
            .map_err(|source| ProjectError::Frontmatter { path, source })?;
        Ok((manifest, text_file.hash))
    }

    /// `kataribe.yaml` を読み、`update` で書き換えて保存し、保存した内容と新しいハッシュを返す。
    ///
    /// 次のときは上書きせずに競合（[`ProjectError::Conflict`]）にする。
    /// - `expected` を渡したのに、今の内容のハッシュがそれと違う（利用者が読んだあとに変わった）。
    /// - 読んでから書くまでの間に、外で変更された。
    ///
    /// 書き直すと、手で書いたコメントや項目の順番は残らない（項目そのものは残る）。
    /// そのため、書き直す前の内容は毎回バックアップに残す。
    pub fn update_manifest<E: From<ProjectError>>(
        &self,
        expected: Option<&ContentHash>,
        update: impl FnOnce(&mut Manifest) -> Result<(), E>,
    ) -> Result<(Manifest, ContentHash), E> {
        let path = RelPath::new(layout::MANIFEST).map_err(ProjectError::from)?;
        let current = self.store.read_text(&path)?;
        if expected.is_some_and(|expected| *expected != current.hash) {
            return Err(ProjectError::Conflict { path }.into());
        }
        let yaml_error = |source| ProjectError::Frontmatter {
            path: path.clone(),
            source,
        };
        let mut manifest = Manifest::parse(&current.content).map_err(yaml_error)?;
        update(&mut manifest)?;
        let text = manifest.render().map_err(yaml_error)?;
        let hash = self.store.write_text(
            &path,
            &text,
            WriteOptions {
                condition: WriteCondition::Matches(current.hash),
                backup: BackupMode::Always,
            },
        )?;
        Ok((manifest, hash))
    }

    /// front matter が任意の Markdown 文書を読み込む。
    pub fn markdown(&self, path: &RelPath) -> Result<Option<MarkdownDoc>, ProjectError> {
        let Some(text_file) = self.store.read_text_opt(path)? else {
            return Ok(None);
        };
        let doc =
            MarkdownDoc::parse(&text_file.content).map_err(|source| ProjectError::Frontmatter {
                path: path.clone(),
                source,
            })?;
        Ok(Some(doc))
    }

    /// ファイルを、画面で編集する形（人物資料・章立ては front matter を項目に分けた形）で読み込む。
    ///
    /// front matter を解釈できない人物資料・章立ては、直して保存できるよう文字列のまま返し、
    /// その理由を [`DocumentFile::parse_error`] に入れる。
    pub fn read_document(&self, path: &RelPath) -> Result<DocumentFile, ProjectError> {
        document::read_document(&self.store, path)
    }

    /// 画面で編集した文書を保存し、保存した内容のハッシュを返す。
    ///
    /// `expected` が `Some` なら、今のファイルのハッシュがそれと一致するときだけ書く。
    /// `None` なら新規作成だけを許す。合わなければ [`ProjectError::Conflict`]。
    ///
    /// 人物資料・章立ては、画面が知らない項目を保存されている側の値で残す。項目が変わっていなければ
    /// 本文だけを差し替え、YAML は書かれたまま（コメントや項目の順番も）残す。項目が変わったときは
    /// YAML を書き直す（コメントや項目の順番は残らない）。
    /// [`EditableDocument::Text`] はどのパスにも書ける。
    pub fn write_document(
        &self,
        path: &RelPath,
        document: &EditableDocument,
        expected: Option<&ContentHash>,
    ) -> Result<ContentHash, ProjectError> {
        document::write_document(&self.store, path, document, expected)
    }

    /// `world/` 配下の Markdown 文書を、`overview.md` を先頭にして名前順で読み込む。
    pub fn world_docs(&self) -> Result<Vec<(RelPath, MarkdownDoc)>, ProjectError> {
        let world_dir = RelPath::new(layout::WORLD_DIR)?;
        if !self.store.exists(&world_dir) {
            return Ok(Vec::new());
        }
        let mut docs = Vec::new();
        for entry in self.store.list_dir(&world_dir)? {
            if entry.kind != DirEntryKind::File || entry.path.extension() != Some("md") {
                continue;
            }
            if let Some(doc) = self.markdown(&entry.path)? {
                docs.push((entry.path, doc));
            }
        }
        docs.sort_by(|(a, _), (b, _)| {
            let a_is_overview = a.as_str() == layout::WORLD_OVERVIEW;
            let b_is_overview = b.as_str() == layout::WORLD_OVERVIEW;
            match (a_is_overview, b_is_overview) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.as_str().cmp(b.as_str()),
            }
        });
        Ok(docs)
    }

    /// 登場人物を `order` → `id` の順で読み込む。
    pub fn characters(&self) -> Result<Vec<Character>, ProjectError> {
        let dir = RelPath::new(layout::CHARACTERS_DIR)?;
        if !self.store.exists(&dir) {
            return Ok(Vec::new());
        }
        let mut characters = Vec::new();
        for entry in self.store.list_dir(&dir)? {
            if entry.kind != DirEntryKind::File {
                continue;
            }
            let DocumentKind::Character(id) = layout::document_kind(&entry.path) else {
                tracing::warn!(path = %entry.path, "登場人物の資料（<id>.md）として解釈できないため無視しました");
                continue;
            };
            if let Some(character) = self.character(&id)? {
                characters.push(character);
            }
        }
        characters.sort_by(|a, b| {
            let order_a = a.meta.order.unwrap_or(u32::MAX);
            let order_b = b.meta.order.unwrap_or(u32::MAX);
            order_a.cmp(&order_b).then_with(|| a.id.cmp(&b.id))
        });
        Ok(characters)
    }

    /// id を指定して登場人物を読み込む。
    pub fn character(&self, id: &CharacterId) -> Result<Option<Character>, ProjectError> {
        let path = layout::character_path(id);
        let Some(text_file) = self.store.read_text_opt(&path)? else {
            return Ok(None);
        };
        let character = Character::parse(id.clone(), &text_file.content)
            .map_err(|source| ProjectError::Frontmatter { path, source })?;
        Ok(Some(character))
    }

    /// 章を id 順で読み込む。`plot/chapters/` の `NN.md` 以外は無視する。
    pub fn chapters(&self) -> Result<Vec<Chapter>, ProjectError> {
        let dir = RelPath::new(layout::CHAPTERS_DIR)?;
        if !self.store.exists(&dir) {
            return Ok(Vec::new());
        }
        let mut chapters = Vec::new();
        for entry in self.store.list_dir(&dir)? {
            if entry.kind != DirEntryKind::File {
                continue;
            }
            let DocumentKind::Chapter(id) = layout::document_kind(&entry.path) else {
                tracing::warn!(path = %entry.path, "章（<NN>.md）として解釈できないため無視しました");
                continue;
            };
            if let Some(chapter) = self.chapter(&id)? {
                chapters.push(chapter);
            }
        }
        chapters.sort_by_key(|chapter| chapter.id);
        Ok(chapters)
    }

    /// id を指定して章を読み込む。
    pub fn chapter(&self, id: &ChapterId) -> Result<Option<Chapter>, ProjectError> {
        let path = layout::chapter_path(id);
        let Some(text_file) = self.store.read_text_opt(&path)? else {
            return Ok(None);
        };
        let chapter = Chapter::parse(*id, &text_file.content)
            .map_err(|source| ProjectError::Frontmatter { path, source })?;
        Ok(Some(chapter))
    }

    /// シーンの本文を読み込む。
    pub fn scene_text(
        &self,
        chapter: &ChapterId,
        scene: &SceneId,
    ) -> Result<Option<String>, ProjectError> {
        let path = layout::scene_text_path(chapter, scene);
        Ok(self
            .store
            .read_text_opt(&path)?
            .map(|text_file| text_file.content))
    }
}

fn ensure_directory_is_creatable(dir: &Path) -> Result<(), ProjectError> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        return Err(ProjectError::DirectoryNotEmpty {
            root: dir.to_path_buf(),
        });
    }
    let entries = fs::read_dir(dir).map_err(|source| ProjectError::RootNotOpenable {
        root: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ProjectError::RootNotOpenable {
            root: dir.to_path_buf(),
            source,
        })?;
        if entry.file_name() != ".git" {
            return Err(ProjectError::DirectoryNotEmpty {
                root: dir.to_path_buf(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use tempfile::TempDir;

    use super::*;
    use crate::model::{ChapterMeta, CharacterMeta, Rating, ScenePlan};
    use crate::store::WriteOptions;

    fn sample_manifest() -> Manifest {
        Manifest {
            format: FORMAT_VERSION,
            title: "みさき館の殺人".to_string(),
            author: Some("沼田".to_string()),
            genre: "mystery".to_string(),
            genre_note: None,
            rating: Rating::General,
            target_length: 30_000,
            idea: "嵐で孤立した岬の洋館で…".to_string(),
            settings: None,
            extra: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn update_manifest_saves_the_change_and_keeps_other_fields() {
        let dir = TempDir::new().unwrap();
        let mut manifest = sample_manifest();
        manifest
            .extra
            .insert("memo".to_string(), serde_json::json!("手で足した項目"));
        let project = Project::create(dir.path(), &manifest).unwrap();

        let (saved, hash) = project
            .update_manifest(None, |manifest| {
                manifest.settings = Some(serde_json::json!({ "provider": "claude_code" }));
                Ok::<_, ProjectError>(())
            })
            .unwrap();

        let path = RelPath::new(layout::MANIFEST).unwrap();
        assert_eq!(project.store().read_text(&path).unwrap().hash, hash);

        assert_eq!(project.manifest().unwrap(), saved);
        assert_eq!(saved.title, manifest.title);
        assert_eq!(saved.extra, manifest.extra);
        assert_eq!(
            saved.settings,
            Some(serde_json::json!({ "provider": "claude_code" }))
        );
    }

    #[test]
    fn update_manifest_does_not_overwrite_an_outside_change() {
        let dir = TempDir::new().unwrap();
        let project = Project::create(dir.path(), &sample_manifest()).unwrap();
        let manifest_path = dir.path().join(layout::MANIFEST);

        let result = project.update_manifest(None, |manifest| {
            // 読んでから書くまでの間に、外で書き換えられた
            let mut changed = manifest.clone();
            changed.title = "外で変えた題名".to_string();
            fs::write(&manifest_path, changed.render().unwrap()).unwrap();
            manifest.settings = Some(serde_json::json!({ "provider": "claude_code" }));
            Ok::<_, ProjectError>(())
        });

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(project.manifest().unwrap().title, "外で変えた題名");
    }

    #[test]
    fn update_manifest_refuses_when_the_file_changed_since_the_caller_read_it() {
        let dir = TempDir::new().unwrap();
        let project = Project::create(dir.path(), &sample_manifest()).unwrap();
        let path = RelPath::new(layout::MANIFEST).unwrap();
        let read_by_caller = project.store().read_text(&path).unwrap().hash;
        project
            .update_manifest(None, |manifest| {
                manifest.title = "別の画面で変えた題名".to_string();
                Ok::<_, ProjectError>(())
            })
            .unwrap();

        let result = project.update_manifest(Some(&read_by_caller), |manifest| {
            manifest.settings = Some(serde_json::json!({ "polish": true }));
            Ok::<_, ProjectError>(())
        });

        assert!(matches!(result, Err(ProjectError::Conflict { .. })));
        assert_eq!(project.manifest().unwrap().settings, None);
    }

    #[test]
    fn update_manifest_writes_nothing_when_the_update_fails() {
        #[derive(Debug)]
        enum UpdateError {
            Project(#[allow(dead_code)] ProjectError),
            Refused,
        }
        impl From<ProjectError> for UpdateError {
            fn from(error: ProjectError) -> Self {
                Self::Project(error)
            }
        }

        let dir = TempDir::new().unwrap();
        let project = Project::create(dir.path(), &sample_manifest()).unwrap();

        let result = project.update_manifest(None, |manifest| {
            manifest.title = "書かれないはずの題名".to_string();
            Err(UpdateError::Refused)
        });

        assert!(matches!(result, Err(UpdateError::Refused)));
        assert_eq!(project.manifest().unwrap().title, sample_manifest().title);
    }

    #[test]
    fn create_then_open_round_trips_manifest() {
        let dir = TempDir::new().unwrap();
        let project_dir = dir.path().join("my-novel");
        let manifest = sample_manifest();

        Project::create(&project_dir, &manifest).unwrap();
        let project = Project::open(&project_dir).unwrap();

        assert_eq!(project.manifest().unwrap(), manifest);
        assert!(project_dir.join(".gitignore").is_file());
        assert!(project_dir.join(".gitattributes").is_file());
        assert!(project_dir.join("world").is_dir());
        assert!(project_dir.join("characters").is_dir());
        assert!(project_dir.join("plot/chapters").is_dir());
        assert!(project_dir.join("manuscript").is_dir());
    }

    #[test]
    fn create_rejects_non_empty_directory() {
        let dir = TempDir::new().unwrap();
        let project_dir = dir.path().join("my-novel");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("existing.txt"), "何か").unwrap();

        let result = Project::create(&project_dir, &sample_manifest());
        assert!(matches!(
            result,
            Err(ProjectError::DirectoryNotEmpty { .. })
        ));
    }

    #[test]
    fn create_allows_directory_with_only_git() {
        let dir = TempDir::new().unwrap();
        let project_dir = dir.path().join("my-novel");
        fs::create_dir_all(project_dir.join(".git")).unwrap();

        assert!(Project::create(&project_dir, &sample_manifest()).is_ok());
    }

    #[test]
    fn open_fails_when_manifest_is_missing() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path()).unwrap();
        let result = Project::open(dir.path());
        assert!(matches!(result, Err(ProjectError::NotAProject { .. })));
    }

    #[test]
    fn open_fails_when_format_is_too_new() {
        let dir = TempDir::new().unwrap();
        let mut manifest = sample_manifest();
        manifest.format = FORMAT_VERSION + 1;

        // create() 自身の最後の open() でも同じ検査にかかるため、ここで失敗する。
        let result = Project::create(dir.path(), &manifest);
        assert!(matches!(
            result,
            Err(ProjectError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn broken_yaml_reports_file_and_line() {
        let dir = TempDir::new().unwrap();
        Project::create(dir.path(), &sample_manifest()).unwrap();
        let project = Project::open(dir.path()).unwrap();

        let broken = "---\nname: [\n---\n本文\n";
        project
            .store()
            .write_text(
                &RelPath::new("characters/rin.md").unwrap(),
                broken,
                WriteOptions::default(),
            )
            .unwrap();

        let result = project.character(&CharacterId::new("rin").unwrap());
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("characters/rin.md"),
            "message was: {message}"
        );
        assert!(message.contains("行目"), "message was: {message}");
    }

    #[test]
    fn characters_are_sorted_by_order_then_id() {
        let dir = TempDir::new().unwrap();
        Project::create(dir.path(), &sample_manifest()).unwrap();
        let project = Project::open(dir.path()).unwrap();

        let rin = Character {
            id: CharacterId::new("rin").unwrap(),
            meta: CharacterMeta {
                name: "霧島 凛".to_string(),
                role: "主人公".to_string(),
                order: Some(1),
                ..Default::default()
            },
            body: String::new(),
        };
        let kenji = Character {
            id: CharacterId::new("kenji").unwrap(),
            meta: CharacterMeta {
                name: "佐藤 健二".to_string(),
                role: "相棒".to_string(),
                order: None,
                ..Default::default()
            },
            body: String::new(),
        };
        let ann = Character {
            id: CharacterId::new("ann").unwrap(),
            meta: CharacterMeta {
                name: "アン".to_string(),
                role: "端役".to_string(),
                order: None,
                ..Default::default()
            },
            body: String::new(),
        };
        for character in [&rin, &kenji, &ann] {
            project
                .store()
                .write_text(
                    &layout::character_path(&character.id),
                    &character.render().unwrap(),
                    WriteOptions::default(),
                )
                .unwrap();
        }

        let loaded = project.characters().unwrap();
        let ids: Vec<&str> = loaded.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["rin", "ann", "kenji"],
            "order 昇順、無い場合は id 昇順で後ろに並ぶはず"
        );
    }

    #[test]
    fn chapters_are_sorted_numerically_and_ignore_unrelated_files() {
        let dir = TempDir::new().unwrap();
        Project::create(dir.path(), &sample_manifest()).unwrap();
        let project = Project::open(dir.path()).unwrap();

        for number in [2, 1, 10] {
            let id = ChapterId::from_number(number);
            let chapter = Chapter {
                id,
                meta: ChapterMeta {
                    title: format!("第{number}章"),
                    ..Default::default()
                },
                storyline: String::new(),
            };
            project
                .store()
                .write_text(
                    &layout::chapter_path(&id),
                    &chapter.render().unwrap(),
                    WriteOptions::default(),
                )
                .unwrap();
        }
        project
            .store()
            .write_text(
                &RelPath::new("plot/chapters/notes.md").unwrap(),
                "無視されるはず",
                WriteOptions::default(),
            )
            .unwrap();

        let chapters = project.chapters().unwrap();
        let numbers: Vec<u32> = chapters.iter().map(|c| c.id.number()).collect();
        assert_eq!(numbers, vec![1, 2, 10]);
    }

    #[test]
    fn scene_text_and_chapter_scene_lookup_round_trip() {
        let dir = TempDir::new().unwrap();
        Project::create(dir.path(), &sample_manifest()).unwrap();
        let project = Project::open(dir.path()).unwrap();

        let chapter_id = ChapterId::from_number(1);
        let scene_id = SceneId::from_number(1);
        let scene = ScenePlan {
            id: scene_id,
            title: "事務所に届いた依頼".to_string(),
            summary: "雨の夜…".to_string(),
            pov: None,
            characters: vec![],
            place: None,
            time: None,
            target_chars: None,
            beats: vec![],
            extra: std::collections::BTreeMap::new(),
        };
        let chapter = Chapter {
            id: chapter_id,
            meta: ChapterMeta {
                title: "雨の匂い".to_string(),
                scenes: vec![scene.clone()],
                extra: std::collections::BTreeMap::new(),
            },
            storyline: "この章のストーリーライン".to_string(),
        };
        project
            .store()
            .write_text(
                &layout::chapter_path(&chapter_id),
                &chapter.render().unwrap(),
                WriteOptions::default(),
            )
            .unwrap();
        project
            .store()
            .write_text(
                &layout::scene_text_path(&chapter_id, &scene_id),
                "本文…",
                WriteOptions::default(),
            )
            .unwrap();

        let loaded_chapter = project.chapter(&chapter_id).unwrap().unwrap();
        assert_eq!(loaded_chapter.scene(&scene_id), Some(&scene));
        assert_eq!(
            project
                .scene_text(&chapter_id, &scene_id)
                .unwrap()
                .as_deref(),
            Some("本文…")
        );
        assert_eq!(
            project
                .scene_text(&chapter_id, &SceneId::from_number(99))
                .unwrap(),
            None
        );
    }

    #[test]
    fn world_docs_puts_overview_first() {
        let dir = TempDir::new().unwrap();
        Project::create(dir.path(), &sample_manifest()).unwrap();
        let project = Project::open(dir.path()).unwrap();

        project
            .store()
            .write_text(
                &RelPath::new("world/glossary.md").unwrap(),
                "# 用語集\n",
                WriteOptions::default(),
            )
            .unwrap();
        project
            .store()
            .write_text(
                &RelPath::new(layout::WORLD_OVERVIEW).unwrap(),
                "# 世界観\n",
                WriteOptions::default(),
            )
            .unwrap();

        let docs = project.world_docs().unwrap();
        let paths: Vec<&str> = docs.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, vec!["world/overview.md", "world/glossary.md"]);
    }
}
