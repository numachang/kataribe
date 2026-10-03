import { computeTextStats } from "../../lib/textStats";
import type { OverviewEntry, OverviewSection, ProjectOverview } from "../types";
import {
  CONCEPT_PATH,
  chapterPath,
  characterPath,
  MANIFEST_PATH,
  STYLE_PATH,
  SYNOPSIS_PATH,
  scenePath,
  WORLD_OVERVIEW_PATH,
  worldDocumentPath,
} from "./paths";
import type { MockScene, ProjectState } from "./state";
import { sortedByNumber, sortedForDisplay, worldDocumentTitle } from "./state";

function charsOf(text: string | null): number {
  return text === null ? 0 : computeTextStats(text).chars;
}

/** 章・シーンに属さない項目は、`chapter` と `scene` を省いてよい。 */
type LeafEntry = Omit<OverviewEntry, "children" | "chapter" | "scene"> &
  Partial<Pick<OverviewEntry, "chapter" | "scene">>;

function leaf(entry: LeafEntry): OverviewEntry {
  return { chapter: null, scene: null, ...entry, children: [] };
}

/** シーンの目標文字数。0 は目標なしとして扱う（Rust の overview と同じ）。 */
function sceneTargetChars(scene: MockScene): number | null {
  return scene.targetChars !== null && scene.targetChars > 0 ? scene.targetChars : null;
}

/**
 * 章の目標文字数。すべてのシーンに目標があるときだけ合計する（Rust の overview と同じ。
 * 一部のシーンだけの合計を、章全体の目標のように見せないため）。
 */
function chapterTargetChars(scenes: MockScene[]): number | null {
  let sum = 0;
  for (const scene of scenes) {
    const target = sceneTargetChars(scene);
    if (target === null) {
      return null;
    }
    sum += target;
  }
  return scenes.length > 0 ? sum : null;
}

function buildPlanningSection(state: ProjectState): OverviewSection {
  return {
    kind: "planning",
    label: "企画",
    entries: [
      leaf({
        path: MANIFEST_PATH,
        label: "作品情報",
        kind: "manifest",
        exists: true,
        chars: 0,
        target_chars: null,
        error: null,
      }),
      leaf({
        path: CONCEPT_PATH,
        label: "企画",
        kind: "concept",
        exists: state.concept !== null,
        chars: charsOf(state.concept),
        target_chars: null,
        error: null,
      }),
      leaf({
        path: STYLE_PATH,
        label: "文体",
        kind: "style",
        exists: state.style !== null,
        chars: charsOf(state.style),
        target_chars: null,
        error: null,
      }),
    ],
  };
}

function buildWorldSection(state: ProjectState): OverviewSection {
  return {
    kind: "world",
    label: "世界観",
    entries: [
      leaf({
        path: WORLD_OVERVIEW_PATH,
        label: "世界観",
        kind: "world",
        exists: state.world !== null,
        chars: charsOf(state.world),
        target_chars: null,
        error: null,
      }),
      ...state.worldDocuments.map((document) =>
        leaf({
          path: worldDocumentPath(document.name),
          label: worldDocumentTitle(document),
          kind: "other",
          exists: true,
          chars: charsOf(document.content),
          target_chars: null,
          error: null,
        }),
      ),
    ],
  };
}

function buildCharactersSection(state: ProjectState): OverviewSection {
  if (!state.characters) {
    return {
      kind: "characters",
      label: "登場人物",
      entries: [
        leaf({
          path: null,
          label: "登場人物",
          kind: "character",
          exists: false,
          chars: 0,
          target_chars: null,
          error: null,
        }),
      ],
    };
  }
  const entries = sortedForDisplay(state.characters).map((character) =>
    leaf({
      path: characterPath(character.id),
      label: character.name,
      kind: "character",
      exists: true,
      chars: charsOf(character.detail),
      target_chars: null,
      error: null,
    }),
  );
  return { kind: "characters", label: "登場人物", entries };
}

function buildPlotSection(state: ProjectState): OverviewSection {
  const synopsisEntry = leaf({
    path: SYNOPSIS_PATH,
    label: "あらすじ",
    kind: "synopsis",
    exists: state.synopsis !== null,
    chars: charsOf(state.synopsis),
    target_chars: null,
    error: null,
  });
  if (!state.chapters) {
    return {
      kind: "plot",
      label: "あらすじ・章立て",
      entries: [
        synopsisEntry,
        leaf({
          path: null,
          label: "章立て",
          kind: "chapter",
          exists: false,
          chars: 0,
          target_chars: null,
          error: null,
        }),
      ],
    };
  }
  // 本物の目次の章も、番号の順（`100` は `99` の後ろ）に並ぶ
  const chapterEntries = sortedByNumber(state.chapters).map((chapter) =>
    leaf({
      path: chapterPath(chapter.id),
      label: chapter.title,
      kind: "chapter",
      chapter: chapter.id,
      exists: true,
      chars: charsOf(chapter.storyline),
      target_chars: null,
      error: null,
    }),
  );
  return { kind: "plot", label: "あらすじ・章立て", entries: [synopsisEntry, ...chapterEntries] };
}

function buildManuscriptSection(state: ProjectState): OverviewSection {
  if (!state.chapters) {
    return {
      kind: "manuscript",
      label: "本文",
      entries: [
        leaf({
          path: null,
          label: "本文",
          kind: "scene",
          exists: false,
          chars: 0,
          target_chars: null,
          error: null,
        }),
      ],
    };
  }
  const entries: OverviewEntry[] = sortedByNumber(state.chapters).map((chapter) => {
    if (!chapter.scenes) {
      return {
        path: null,
        label: chapter.title,
        kind: "chapter",
        chapter: chapter.id,
        scene: null,
        exists: true,
        chars: 0,
        target_chars: null,
        error: null,
        children: [
          leaf({
            path: null,
            label: "シーン構成が未生成です",
            kind: "scene",
            exists: false,
            chars: 0,
            target_chars: null,
            error: null,
          }),
        ],
      };
    }
    const sceneEntries = chapter.scenes.map((scene) =>
      leaf({
        path: scenePath(chapter.id, scene.id),
        label: scene.title,
        kind: "scene",
        chapter: chapter.id,
        scene: scene.id,
        exists: scene.draft !== null,
        chars: charsOf(scene.draft),
        target_chars: sceneTargetChars(scene),
        error: null,
      }),
    );
    return {
      path: null,
      label: chapter.title,
      kind: "chapter",
      chapter: chapter.id,
      scene: null,
      exists: true,
      chars: sceneEntries.reduce((sum, entry) => sum + entry.chars, 0),
      target_chars: chapterTargetChars(chapter.scenes),
      error: null,
      children: sceneEntries,
    };
  });
  return { kind: "manuscript", label: "本文", entries };
}

function countManuscriptChars(state: ProjectState): number {
  if (!state.chapters) {
    return 0;
  }
  return state.chapters.reduce((chapterSum, chapter) => {
    const sceneSum = (chapter.scenes ?? []).reduce((sum, scene) => sum + charsOf(scene.draft), 0);
    return chapterSum + sceneSum;
  }, 0);
}

/** 現在の状態から目次（ProjectOverview）を組み立てる。 */
export function buildOverview(state: ProjectState): ProjectOverview {
  return {
    root: state.folder,
    title: state.manifest.title,
    target_length: state.manifest.target_length,
    total_chars: countManuscriptChars(state),
    sections: [
      buildPlanningSection(state),
      buildWorldSection(state),
      buildCharactersSection(state),
      buildPlotSection(state),
      buildManuscriptSection(state),
    ],
  };
}
