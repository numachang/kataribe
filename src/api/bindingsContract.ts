// 手書きの契約（./types.ts と ./backend.ts）が、Rust の型から ts-rs が生成した型（../bindings）と
// 完全に一致することを、型検査（pnpm typecheck）で保証する。Rust 側の型を変えて bindings を
// 作り直したのに手書きの型を直し忘れると、ここが型エラーになる。
// bindings 自体が Rust の型と一致していることは、CI で cargo test の後に src/bindings に
// 差分が無いことで確かめる。

import type { AppSettings as GeneratedAppSettings } from "../bindings/AppSettings";
import type { ChangeSet as GeneratedChangeSet } from "../bindings/ChangeSet";
import type { CommandErrorKind as GeneratedCommandErrorKind } from "../bindings/CommandErrorKind";
import type { DraftUnit as GeneratedDraftUnit } from "../bindings/DraftUnit";
import type { EditorPreferences as GeneratedEditorPreferences } from "../bindings/EditorPreferences";
import type { EntryKind as GeneratedEntryKind } from "../bindings/EntryKind";
import type { FileChange as GeneratedFileChange } from "../bindings/FileChange";
import type { FontStyle as GeneratedFontStyle } from "../bindings/FontStyle";
import type { GenerationEvent as GeneratedGenerationEvent } from "../bindings/GenerationEvent";
import type { GenerationSettings as GeneratedGenerationSettings } from "../bindings/GenerationSettings";
import type { GenrePreset as GeneratedGenrePreset } from "../bindings/GenrePreset";
import type { IssueKind as GeneratedIssueKind } from "../bindings/IssueKind";
import type { LlmSettings as GeneratedLlmSettings } from "../bindings/LlmSettings";
import type { Manifest as GeneratedManifest } from "../bindings/Manifest";
import type { ModelInfo as GeneratedModelInfo } from "../bindings/ModelInfo";
import type { NewProject as GeneratedNewProject } from "../bindings/NewProject";
import type { OverviewEntry as GeneratedOverviewEntry } from "../bindings/OverviewEntry";
import type { OverviewSection as GeneratedOverviewSection } from "../bindings/OverviewSection";
import type { PipelineStep as GeneratedPipelineStep } from "../bindings/PipelineStep";
import type { ProjectOverview as GeneratedProjectOverview } from "../bindings/ProjectOverview";
import type { QualityIssue as GeneratedQualityIssue } from "../bindings/QualityIssue";
import type { QualityMetrics as GeneratedQualityMetrics } from "../bindings/QualityMetrics";
import type { QualityReport as GeneratedQualityReport } from "../bindings/QualityReport";
import type { Rating as GeneratedRating } from "../bindings/Rating";
import type { SectionKind as GeneratedSectionKind } from "../bindings/SectionKind";
import type { Segment as GeneratedSegment } from "../bindings/Segment";
import type { Severity as GeneratedSeverity } from "../bindings/Severity";
import type { StepState as GeneratedStepState } from "../bindings/StepState";
import type { Task as GeneratedTask } from "../bindings/Task";
import type { TextFile as GeneratedTextFile } from "../bindings/TextFile";
import type { TextStats as GeneratedTextStats } from "../bindings/TextStats";
import type { BackendErrorKind } from "./backend";
import type * as Api from "./types";

/** A と B が同じ型のときだけ true になる（一方が他方に代入できるだけでは false）。 */
type Equal<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
/** 型引数が true でなければ型エラーにする。 */
type Expect<Condition extends true> = Condition;

export type BindingsContract = [
  Expect<Equal<Api.Rating, GeneratedRating>>,
  Expect<Equal<Api.Manifest, GeneratedManifest>>,
  Expect<Equal<Api.NewProject, GeneratedNewProject>>,
  Expect<Equal<Api.GenrePreset, GeneratedGenrePreset>>,
  Expect<Equal<Api.SectionKind, GeneratedSectionKind>>,
  Expect<Equal<Api.EntryKind, GeneratedEntryKind>>,
  Expect<Equal<Api.OverviewEntry, GeneratedOverviewEntry>>,
  Expect<Equal<Api.OverviewSection, GeneratedOverviewSection>>,
  Expect<Equal<Api.ProjectOverview, GeneratedProjectOverview>>,
  Expect<Equal<Api.TextFile, GeneratedTextFile>>,
  Expect<Equal<Api.TextStats, GeneratedTextStats>>,
  Expect<Equal<Api.Segment, GeneratedSegment>>,
  Expect<Equal<Api.Severity, GeneratedSeverity>>,
  Expect<Equal<Api.IssueKind, GeneratedIssueKind>>,
  Expect<Equal<Api.QualityIssue, GeneratedQualityIssue>>,
  Expect<Equal<Api.QualityMetrics, GeneratedQualityMetrics>>,
  Expect<Equal<Api.QualityReport, GeneratedQualityReport>>,
  Expect<Equal<Api.DraftUnit, GeneratedDraftUnit>>,
  Expect<Equal<Api.LlmSettings, GeneratedLlmSettings>>,
  Expect<Equal<Api.GenerationSettings, GeneratedGenerationSettings>>,
  Expect<Equal<Api.FontStyle, GeneratedFontStyle>>,
  Expect<Equal<Api.EditorPreferences, GeneratedEditorPreferences>>,
  Expect<Equal<Api.AppSettings, GeneratedAppSettings>>,
  Expect<Equal<Api.ModelInfo, GeneratedModelInfo>>,
  Expect<Equal<Api.Task, GeneratedTask>>,
  Expect<Equal<Api.StepState, GeneratedStepState>>,
  Expect<Equal<Api.PipelineStep, GeneratedPipelineStep>>,
  Expect<Equal<Api.GenerationEvent, GeneratedGenerationEvent>>,
  Expect<Equal<Api.FileChange, GeneratedFileChange>>,
  Expect<Equal<Api.ChangeSet, GeneratedChangeSet>>,
  Expect<Equal<BackendErrorKind, GeneratedCommandErrorKind>>,
];

// Equal が緩すぎないこと（null を許すかどうかの違いを見逃さないこと）の確認。
// @ts-expect-error 片方にしか null が無い型は一致とみなさない
export type EqualRejectsWiderType = Expect<Equal<{ a: string }, { a: string | null }>>;
