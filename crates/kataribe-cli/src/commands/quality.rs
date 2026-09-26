//! `quality` サブコマンド: シーンごとの品質レポートを表示する。

use std::fmt::Write as _;

use anyhow::Context;
use kataribe_project::Project;
use kataribe_text::quality::{self, QualityOptions, QualityReport, Severity};
use serde_json::json;

use crate::args::QualityArgs;
use crate::output::Console;

use super::Outcome;

/// 1 シーン分の品質レポート。
struct SceneQuality {
    chapter_number: u32,
    chapter_title: String,
    scene_id: String,
    scene_title: String,
    report: QualityReport,
}

pub fn run(args: &QualityArgs, console: &dyn Console) -> anyhow::Result<Outcome> {
    let project = Project::open(&args.folder).context("作品フォルダを開けません")?;
    let reports = collect_reports(&project)?;

    if args.json {
        let payload: Vec<_> = reports.iter().map(as_json).collect();
        console.print(&serde_json::to_string_pretty(&payload)?);
        console.print("\n");
    } else {
        console.print(&render(&reports));
    }
    Ok(Outcome::Success)
}

fn collect_reports(project: &Project) -> anyhow::Result<Vec<SceneQuality>> {
    let mut reports = Vec::new();
    for chapter in project.chapters().context("章の一覧を取得できません")? {
        for scene in &chapter.meta.scenes {
            let Some(text) = project.scene_text(&chapter.id, &scene.id)? else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            let options = QualityOptions {
                target_chars: scene.target_chars.map(|chars| chars as usize),
            };
            reports.push(SceneQuality {
                chapter_number: chapter.id.number(),
                chapter_title: chapter.meta.title.clone(),
                scene_id: scene.id.to_string(),
                scene_title: scene.title.clone(),
                report: quality::analyze(&text, &options),
            });
        }
    }
    Ok(reports)
}

fn as_json(item: &SceneQuality) -> serde_json::Value {
    json!({
        "chapter_number": item.chapter_number,
        "chapter_title": item.chapter_title,
        "scene_id": item.scene_id,
        "scene_title": item.scene_title,
        "report": item.report,
    })
}

fn render(reports: &[SceneQuality]) -> String {
    if reports.is_empty() {
        return "本文はまだありません。\n".to_owned();
    }
    let mut rendered = String::new();
    for item in reports {
        // String への write! は失敗しないため、戻り値は無視してよい。
        let _ = writeln!(
            rendered,
            "第{}章「{}」 {}「{}」  {} 文字（会話 {:.0}%）",
            item.chapter_number,
            item.chapter_title,
            item.scene_id,
            item.scene_title,
            item.report.stats.chars,
            item.report.metrics.dialogue_ratio * 100.0
        );
        for issue in &item.report.issues {
            let _ = writeln!(
                rendered,
                "  - [{}] {}",
                severity_label(issue.severity),
                issue.message
            );
        }
    }
    rendered
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "情報",
        Severity::Warning => "注意",
        Severity::Error => "エラー",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kataribe_text::count::TextStats;
    use kataribe_text::quality::{IssueKind, QualityIssue, QualityMetrics};

    fn sample_report(chars: usize) -> QualityReport {
        QualityReport {
            stats: TextStats {
                chars,
                paragraphs: 1,
                dialogue_lines: 0,
                manuscript_pages: 0.0,
            },
            metrics: QualityMetrics {
                dialogue_ratio: 0.5,
                kanji_ratio: 0.3,
                average_sentence_length: 20.0,
                longest_same_ending_run: 0,
                repeated_phrase_ratio: 0.0,
            },
            issues: vec![QualityIssue {
                kind: IssueKind::TooShort,
                severity: Severity::Warning,
                message: "目標文字数の 7 割未満です。".to_owned(),
                excerpt: None,
            }],
        }
    }

    #[test]
    fn render_includes_chapter_scene_and_issue_message() {
        let reports = vec![SceneQuality {
            chapter_number: 1,
            chapter_title: "雨の匂い".to_owned(),
            scene_id: "s01".to_owned(),
            scene_title: "事務所に届いた依頼".to_owned(),
            report: sample_report(120),
        }];

        let rendered = render(&reports);

        assert!(rendered.contains("第1章「雨の匂い」"));
        assert!(rendered.contains("s01「事務所に届いた依頼」"));
        assert!(rendered.contains("120 文字"));
        assert!(rendered.contains("[注意] 目標文字数の 7 割未満です。"));
    }

    #[test]
    fn render_reports_when_nothing_is_written_yet() {
        assert_eq!(render(&[]), "本文はまだありません。\n");
    }
}
