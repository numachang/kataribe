import type { PipelineStep, StepState, Task } from "../types";
import type { ProjectState } from "./state";

function step(task: Task, label: string, state: StepState, blockedBy: string | null): PipelineStep {
  return { task, label, state, blocked_by: blockedBy };
}

function doneOrReady(done: boolean): StepState {
  return done ? "done" : "ready";
}

/** 現在の状態から工程（PipelineStep の一覧）を組み立てる。 */
export function buildPipeline(state: ProjectState): PipelineStep[] {
  const steps: PipelineStep[] = [];

  steps.push(step({ kind: "concept" }, "企画", doneOrReady(state.concept !== null), null));

  const conceptBlock = state.concept === null ? "先に「企画」を生成してください" : null;
  steps.push(
    step(
      { kind: "style" },
      "文体",
      conceptBlock ? "blocked" : doneOrReady(state.style !== null),
      conceptBlock,
    ),
  );
  steps.push(
    step(
      { kind: "world" },
      "世界観",
      conceptBlock ? "blocked" : doneOrReady(state.world !== null),
      conceptBlock,
    ),
  );

  const castBlock =
    state.world === null || state.style === null
      ? "先に「世界観」と「文体」を生成してください"
      : null;
  steps.push(
    step(
      { kind: "cast" },
      "登場人物一覧",
      castBlock ? "blocked" : doneOrReady(state.characters !== null),
      castBlock,
    ),
  );

  for (const character of state.characters ?? []) {
    steps.push(
      step(
        { kind: "character", id: character.id },
        `登場人物: ${character.name}`,
        doneOrReady((character.detail ?? "").trim() !== ""),
        null,
      ),
    );
  }

  const synopsisBlock = state.characters === null ? "先に「登場人物一覧」を生成してください" : null;
  steps.push(
    step(
      { kind: "synopsis" },
      "あらすじ",
      synopsisBlock ? "blocked" : doneOrReady(state.synopsis !== null),
      synopsisBlock,
    ),
  );

  const outlineBlock = state.synopsis === null ? "先に「あらすじ」を生成してください" : null;
  steps.push(
    step(
      { kind: "outline" },
      "章立て",
      outlineBlock ? "blocked" : doneOrReady(state.chapters !== null),
      outlineBlock,
    ),
  );

  for (const chapter of state.chapters ?? []) {
    steps.push(
      step(
        { kind: "scene_plan", chapter: chapter.id },
        `${chapter.title}のシーン構成`,
        doneOrReady(chapter.scenes !== null),
        null,
      ),
    );

    for (const scene of chapter.scenes ?? []) {
      steps.push(
        step(
          { kind: "draft", chapter: chapter.id, scene: scene.id },
          `${chapter.title} ${scene.title}`,
          doneOrReady(scene.draft !== null),
          null,
        ),
      );
    }
  }

  return steps;
}
