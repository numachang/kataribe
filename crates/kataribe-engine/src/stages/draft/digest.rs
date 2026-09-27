//! 「これまでの物語」の要約。書き終えたシーンを LLM に要約させ、本文のハッシュをキーにキャッシュする。
//! 本文が手で書き換えられればハッシュが変わるので、要約も自動で作り直される。

use std::collections::BTreeMap;

use kataribe_project::{BackupMode, ContentHash, RelPath, WriteCondition, WriteOptions, layout};
use minijinja::context;
use serde::{Deserialize, Serialize};

use super::story::{Position, Story};
use crate::cleanup::clean_plain;
use crate::error::Result;
use crate::events::{NoticeLevel, notice};
use crate::excerpt;
use crate::prompt::PromptTemplate;
use crate::stages::Stage;
use crate::stages::materials::chapter_label;

const CACHE_FILE: &str = "digests.json";
const DIGEST_MAX_CHARS: u32 = 250;
const DIGEST_OUTPUT_TOKENS: u32 = 1024;
const STORYLINE_CHARS: usize = 300;
/// 要約させる本文の上限（これより長い本文は末尾を省く）。
const SOURCE_CHARS: usize = 8000;

/// プロンプトに入る「これまでの物語」の 1 項目。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct StoryEntry {
    pub label: String,
    pub summary: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DigestCache {
    entries: BTreeMap<String, CachedDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDigest {
    hash: ContentHash,
    summary: String,
}

/// 書き終えたシーンのうち、要約がまだ無いもの。
struct PendingDigest {
    /// 要約を書き込む `entries` の添字。
    entry_index: usize,
    key: String,
    hash: ContentHash,
    title: String,
    text: String,
}

/// `position` より前の物語を、古い章は章のストーリーライン、直前の章と現在の章はシーンごとの要約で表す。
pub(crate) async fn story_so_far(
    stage: &Stage<'_>,
    story: &Story,
    position: Position,
) -> Result<Vec<StoryEntry>> {
    let mut cache = load_cache(stage);
    let detailed_from = position.chapter.saturating_sub(1);
    let mut entries = Vec::new();
    let mut pending = Vec::new();

    for (chapter_index, chapter) in story.chapters.iter().enumerate().take(position.chapter + 1) {
        if chapter_index < detailed_from {
            entries.push(StoryEntry {
                label: chapter_label(chapter),
                summary: excerpt::head(&chapter.storyline, STORYLINE_CHARS),
            });
            continue;
        }
        let scene_end = if chapter_index == position.chapter {
            position.scene
        } else {
            chapter.meta.scenes.len()
        };
        for scene in &chapter.meta.scenes[..scene_end] {
            let label = format!("{} {}", chapter_label(chapter), scene.title);
            let key = format!("{}/{}", chapter.id, scene.id);
            let path = layout::scene_text_path(&chapter.id, &scene.id);
            let summary = match stage.project.store().read_text_opt(&path)? {
                Some(file) if !file.content.trim().is_empty() => match cache.entries.get(&key) {
                    Some(cached) if cached.hash == file.hash => cached.summary.clone(),
                    _ => {
                        pending.push(PendingDigest {
                            entry_index: entries.len(),
                            key,
                            hash: file.hash,
                            title: scene.title.clone(),
                            text: file.content,
                        });
                        String::new()
                    }
                },
                _ => format!("（予定）{}", scene.summary),
            };
            entries.push(StoryEntry { label, summary });
        }
    }

    if pending.is_empty() {
        return Ok(entries);
    }
    stage
        .caller
        .expect_steps(u32::try_from(pending.len()).unwrap_or(u32::MAX));
    for digest in pending {
        let summary = summarize(stage, &digest.title, &digest.text).await?;
        entries[digest.entry_index].summary.clone_from(&summary);
        cache.entries.insert(
            digest.key,
            CachedDigest {
                hash: digest.hash,
                summary,
            },
        );
    }
    save_cache(stage, &cache);
    Ok(entries)
}

async fn summarize(stage: &Stage<'_>, title: &str, text: &str) -> Result<String> {
    let prompt = stage.prompts.render(
        PromptTemplate::Digest,
        &context! {
            project => &stage.info,
            scene_title => title,
            text => excerpt::head(text, SOURCE_CHARS),
            max_chars => DIGEST_MAX_CHARS,
        },
    )?;
    let output_tokens = stage.output_tokens(&prompt, DIGEST_OUTPUT_TOKENS)?;
    let output = stage
        .caller
        .text(&format!("「{title}」を要約"), &prompt, output_tokens)
        .await?;
    Ok(clean_plain(&output.text))
}

fn cache_path() -> Option<RelPath> {
    RelPath::new(layout::CACHE_DIR).ok()?.join(CACHE_FILE).ok()
}

/// キャッシュは作り直せるデータなので、読めなくても生成は止めない。
fn load_cache(stage: &Stage<'_>) -> DigestCache {
    let Some(path) = cache_path() else {
        return DigestCache::default();
    };
    match stage.project.store().read_text_opt(&path) {
        Ok(Some(file)) => serde_json::from_str(&file.content).unwrap_or_else(|error| {
            tracing::warn!(%error, "要約のキャッシュを読めないため作り直します");
            DigestCache::default()
        }),
        Ok(None) => DigestCache::default(),
        Err(error) => {
            tracing::warn!(%error, "要約のキャッシュを読めないため作り直します");
            DigestCache::default()
        }
    }
}

fn save_cache(stage: &Stage<'_>, cache: &DigestCache) {
    let result = cache_path()
        .ok_or_else(|| "キャッシュのパスが不正です".to_owned())
        .and_then(|path| {
            let json = serde_json::to_string_pretty(cache).map_err(|error| error.to_string())?;
            stage
                .project
                .store()
                .write_text(
                    &path,
                    &json,
                    WriteOptions {
                        condition: WriteCondition::Any,
                        backup: BackupMode::Never,
                    },
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        notice(
            stage.caller.sink(),
            NoticeLevel::Warning,
            format!("要約のキャッシュを保存できませんでした（次回また要約します）: {error}"),
        );
    }
}
