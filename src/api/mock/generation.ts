import { toGraphemes } from "../../lib/graphemes";
import { BackendError } from "../backend";
import type { ChangeSet, GenerationEvent, GenerationSettings, Task } from "../types";

import {
  generateCastDrafts,
  generateCharacterDetail,
  generateConceptText,
  generateDraftFullText,
  generateDraftParagraphsForBeat,
  generateOutlineChapters,
  generateRevisionText,
  generateScenePlan,
  generateStyleText,
  generateSynopsisText,
  generateWorldText,
  splitIntoBeats,
} from "./content";
import { writeChange } from "./fileChange";
import {
  CONCEPT_PATH,
  chapterPath,
  characterPath,
  STYLE_PATH,
  SYNOPSIS_PATH,
  scenePath,
  WORLD_OVERVIEW_PATH,
} from "./paths";
import { readMockFile, renderChapterFile, renderCharacterFile } from "./render";
import type { MockChapter, MockScene, ProjectState } from "./state";
import { findChapter, findCharacter, findScene } from "./state";

/** 生成した変更案のうち、作品フォルダ（`project_root`）を付ける前のもの。 */
export type GeneratedChanges = Omit<ChangeSet, "project_root">;

/** キャンセルされた生成が投げる合図。呼び出し側は BackendError("cancelled") に変換する。 */
export class GenerationCancelled extends Error {
  constructor() {
    super("生成を中止しました");
    this.name = "GenerationCancelled";
  }
}

/** 実行中の生成 1 件分。setTimeout ベースのストリーミングを、途中で打ち切れるようにする。 */
export interface GenerationJob {
  cancel(): void;
  sleep(ms: number): Promise<void>;
}

export function createGenerationJob(): GenerationJob {
  let cancelled = false;
  let pendingReject: ((error: Error) => void) | null = null;
  let pendingHandle: ReturnType<typeof setTimeout> | null = null;

  return {
    cancel(): void {
      cancelled = true;
      if (pendingHandle !== null) {
        clearTimeout(pendingHandle);
        pendingHandle = null;
      }
      if (pendingReject) {
        const reject = pendingReject;
        pendingReject = null;
        reject(new GenerationCancelled());
      }
    },
    async sleep(ms: number): Promise<void> {
      if (cancelled) {
        throw new GenerationCancelled();
      }
      if (ms <= 0) {
        return;
      }
      await new Promise<void>((resolve, reject) => {
        pendingReject = reject;
        pendingHandle = setTimeout(() => {
          pendingReject = null;
          pendingHandle = null;
          resolve();
        }, ms);
      });
    },
  };
}

interface StepPlan {
  label: string;
  reasoning: string[];
  chunks: string[];
  notices: Array<{ level: "info" | "warning"; message: string }>;
}

function estimateTokens(text: string): number {
  return Math.max(1, Math.round(toGraphemes(text).length * 0.6));
}

function chunkText(text: string, size = 14): string[] {
  const graphemes = toGraphemes(text);
  const chunks: string[] = [];
  for (let index = 0; index < graphemes.length; index += size) {
    chunks.push(graphemes.slice(index, index + size).join(""));
  }
  return chunks.length > 0 ? chunks : [""];
}

async function runStep(
  job: GenerationJob,
  plan: StepPlan,
  index: number,
  total: number,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<string> {
  onEvent({ kind: "step_started", label: plan.label, index, total });
  for (const reasoning of plan.reasoning) {
    await job.sleep(delayMs);
    onEvent({ kind: "reasoning", text: reasoning });
  }
  let assembled = "";
  for (const chunk of plan.chunks) {
    await job.sleep(delayMs);
    onEvent({ kind: "content", text: chunk });
    assembled += chunk;
  }
  for (const notice of plan.notices) {
    await job.sleep(delayMs);
    onEvent({ kind: "notice", level: notice.level, message: notice.message });
  }
  await job.sleep(delayMs);
  onEvent({
    kind: "step_finished",
    prompt_tokens: estimateTokens(plan.label),
    completion_tokens: estimateTokens(assembled),
    elapsed_ms: (plan.chunks.length + plan.reasoning.length + 1) * Math.max(delayMs, 1),
  });
  return assembled;
}

async function runSteps(
  job: GenerationJob,
  plans: StepPlan[],
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<string[]> {
  const total = plans.length;
  const results: string[] = [];
  for (const [index, plan] of plans.entries()) {
    results.push(await runStep(job, plan, index + 1, total, delayMs, onEvent));
  }
  return results;
}

function notFound(message: string): BackendError {
  return new BackendError("not_found", message);
}

function requireChapter(state: ProjectState, chapterId: string): MockChapter {
  const chapter = findChapter(state, chapterId);
  if (!chapter) {
    throw notFound(`章「${chapterId}」が見つかりません。`);
  }
  return chapter;
}

function requireScene(state: ProjectState, chapterId: string, sceneId: string): MockScene {
  const scene = findScene(state, chapterId, sceneId);
  if (!scene) {
    throw notFound(`シーン「${chapterId}/${sceneId}」が見つかりません。`);
  }
  return scene;
}

async function generateConcept(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const text = generateConceptText(state.manifest);
  const plan: StepPlan = {
    label: "企画を生成しています",
    reasoning: ["着想の芯を整理しています"],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  return {
    summary: "企画を生成しました",
    files: [writeChange(state, CONCEPT_PATH, assembled ?? text)],
  };
}

async function generateStyle(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const text = generateStyleText(state.manifest);
  const plan: StepPlan = {
    label: "文体を生成しています",
    reasoning: [],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  return {
    summary: "文体を生成しました",
    files: [writeChange(state, STYLE_PATH, assembled ?? text)],
  };
}

async function generateWorld(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const text = generateWorldText(state.manifest);
  const plan: StepPlan = {
    label: "世界観を生成しています",
    reasoning: [],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  return {
    summary: "世界観を生成しました",
    files: [writeChange(state, WORLD_OVERVIEW_PATH, assembled ?? text)],
  };
}

async function generateCast(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const drafts = generateCastDrafts();
  const preview = drafts
    .map((draft, index) => `${index + 1}. ${draft.name}（${draft.role}）―― ${draft.summary}`)
    .join("\n");
  const plan: StepPlan = {
    label: "登場人物一覧を生成しています",
    reasoning: ["物語に必要な役割を洗い出しています"],
    chunks: chunkText(preview),
    notices: [{ level: "info", message: "各人物の詳細は、この後の工程で個別に生成します。" }],
  };
  await runSteps(job, [plan], delayMs, onEvent);
  const files = drafts.map((draft, index) =>
    writeChange(
      state,
      characterPath(draft.id),
      renderCharacterFile({ ...draft, order: index + 1, detail: null }),
    ),
  );
  return { summary: "登場人物一覧を生成しました", files };
}

async function generateCharacter(
  state: ProjectState,
  id: string,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const character = findCharacter(state, id);
  if (!character) {
    throw notFound(`登場人物「${id}」が見つかりません。`);
  }
  const text = generateCharacterDetail(character);
  const plan: StepPlan = {
    label: `${character.name}の詳細を生成しています`,
    reasoning: [],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  const file = writeChange(
    state,
    characterPath(id),
    renderCharacterFile({ ...character, detail: assembled ?? text }),
  );
  return { summary: `${character.name}の詳細を生成しました`, files: [file] };
}

async function generateSynopsis(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const text = generateSynopsisText(state.manifest, state.characters ?? []);
  const plan: StepPlan = {
    label: "あらすじを生成しています",
    reasoning: ["登場人物の動機を整理しています"],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  return {
    summary: "あらすじを生成しました",
    files: [writeChange(state, SYNOPSIS_PATH, assembled ?? text)],
  };
}

async function generateOutline(
  state: ProjectState,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const chapters = generateOutlineChapters();
  const preview = chapters.map((chapter) => `${chapter.title}\n${chapter.storyline}`).join("\n\n");
  const plan: StepPlan = {
    label: "章立てを生成しています",
    reasoning: ["物語の山場を配置しています"],
    chunks: chunkText(preview),
    notices: [],
  };
  await runSteps(job, [plan], delayMs, onEvent);
  const files = chapters.map((chapter) =>
    writeChange(state, chapterPath(chapter.id), renderChapterFile({ ...chapter, scenes: null })),
  );
  return { summary: "章立てを生成しました", files };
}

async function generateScenePlanTask(
  state: ProjectState,
  chapterId: string,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const chapter = requireChapter(state, chapterId);
  const scenes = generateScenePlan(chapter);
  const preview = scenes
    .map((scene) => `${scene.id} ${scene.title} ―― ${scene.summary}`)
    .join("\n");
  const plan: StepPlan = {
    label: `${chapter.title}のシーン構成を生成しています`,
    reasoning: ["章のストーリーラインをシーン単位に分割しています"],
    chunks: chunkText(preview),
    notices: [],
  };
  await runSteps(job, [plan], delayMs, onEvent);
  const fullScenes = scenes.map((scene) => ({ ...scene, beats: null, draft: null }));
  const file = writeChange(
    state,
    chapterPath(chapter.id),
    renderChapterFile({ ...chapter, scenes: fullScenes }),
  );
  return { summary: `${chapter.title}のシーン構成を生成しました`, files: [file] };
}

async function generateDraft(
  state: ProjectState,
  chapterId: string,
  sceneId: string,
  settings: GenerationSettings,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const chapter = requireChapter(state, chapterId);
  const scene = requireScene(state, chapterId, sceneId);

  const plans: StepPlan[] =
    settings.draft_unit === "beat"
      ? (scene.beats ?? splitIntoBeats(scene)).map((beat, index, beats) => ({
          label: `${chapter.title} ${scene.title}（${index + 1}/${beats.length}）`,
          reasoning: index === 0 ? ["視点人物の感覚を確認しています"] : [],
          chunks: chunkText(generateDraftParagraphsForBeat(scene, beat, index).join("\n")),
          notices: [],
        }))
      : [
          {
            label: `${chapter.title} ${scene.title}を生成しています`,
            reasoning: ["場面の空気を確認しています"],
            chunks: chunkText(generateDraftFullText(scene)),
            notices:
              scene.targetChars !== null && scene.targetChars > settings.chars_per_call
                ? [
                    {
                      level: "info" as const,
                      message: "目安の分量に収まるよう、続きは次回の生成で書き継ぎます。",
                    },
                  ]
                : [],
          },
        ];

  const assembled = await runSteps(job, plans, delayMs, onEvent);
  const content = assembled.join("\n");
  const file = writeChange(state, scenePath(chapter.id, scene.id), content);
  return { summary: `${chapter.title} ${scene.title}の本文を生成しました`, files: [file] };
}

async function generateRevision(
  state: ProjectState,
  path: string,
  instruction: string,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  const original = readMockFile(state, path);
  if (original === null) {
    throw notFound(`「${path}」はまだ生成されていないため、書き直せません。`);
  }
  const text = generateRevisionText(original, instruction);
  const plan: StepPlan = {
    label: "指示に沿って書き直しています",
    reasoning: [`指示「${instruction}」の意図を確認しています`],
    chunks: chunkText(text),
    notices: [],
  };
  const [assembled] = await runSteps(job, [plan], delayMs, onEvent);
  return {
    summary: "指示に沿って書き直しました",
    files: [writeChange(state, path, assembled ?? text)],
  };
}

/** タスクを実行し、ストリーミングイベントを届けながら変更案を作る。状態そのものは書き換えない。 */
export async function runGeneration(
  state: ProjectState,
  task: Task,
  settings: GenerationSettings,
  job: GenerationJob,
  delayMs: number,
  onEvent: (event: GenerationEvent) => void,
): Promise<GeneratedChanges> {
  switch (task.kind) {
    case "concept":
      return generateConcept(state, job, delayMs, onEvent);
    case "style":
      return generateStyle(state, job, delayMs, onEvent);
    case "world":
      return generateWorld(state, job, delayMs, onEvent);
    case "cast":
      return generateCast(state, job, delayMs, onEvent);
    case "character":
      return generateCharacter(state, task.id, job, delayMs, onEvent);
    case "synopsis":
      return generateSynopsis(state, job, delayMs, onEvent);
    case "outline":
      return generateOutline(state, job, delayMs, onEvent);
    case "scene_plan":
      return generateScenePlanTask(state, task.chapter, job, delayMs, onEvent);
    case "draft":
      return generateDraft(state, task.chapter, task.scene, settings, job, delayMs, onEvent);
    case "revise":
      return generateRevision(state, task.path, task.instruction, job, delayMs, onEvent);
    default:
      return task satisfies never;
  }
}
