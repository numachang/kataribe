//! 作品フォルダの構成・安全なファイル操作・設定資料のモデル。
//!
//! この crate の最優先事項は **利用者の原稿を絶対に失わない・壊さない** こと。
//! 書き込みはアトミックに行い、上書き前にはバックアップを残し、削除は
//! ゴミ箱へ移すだけにする（[`store`]）。作品フォルダの外へは、シンボリックリンクを
//! 使っても出られない（[`path`]、[`store`]）。

mod document;
mod error;
pub mod frontmatter;
pub mod layout;
pub mod model;
pub mod path;
pub mod project;
pub mod store;

pub use document::{EditableDocument, LoadedDocument};
pub use error::ProjectError;
pub use frontmatter::YamlError;
pub use model::{
    Chapter, ChapterId, ChapterMeta, Character, CharacterId, CharacterMeta, DocMeta,
    FORMAT_VERSION, Manifest, MarkdownDoc, ModelError, Rating, SceneId, ScenePlan,
};
pub use path::{PathError, RelPath};
pub use project::Project;
pub use store::{
    BackupMode, ContentHash, PendingWrite, ProjectStore, TextFile, WriteCondition, WriteOptions,
    normalize_text,
};
