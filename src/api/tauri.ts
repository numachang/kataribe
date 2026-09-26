import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend, BackendErrorKind } from "./backend";
import { BackendError } from "./backend";
import type {
  AppSettings,
  ChangeSet,
  GenerationEvent,
  GenrePreset,
  LlmSettings,
  ModelInfo,
  NewProject,
  PipelineStep,
  ProjectOverview,
  QualityReport,
  Segment,
  Task,
  TextFile,
  TextStats,
} from "./types";

// docs/architecture.md §5.1 のコマンド名・引数のとおりに呼び出す。
// Rust 側は snake_case の引数名で受けるため、invoke に渡すオブジェクトのキーも snake_case にする。

interface CommandFailure {
  kind: BackendErrorKind;
  message: string;
}

function isCommandFailure(value: unknown): value is CommandFailure {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as { kind: unknown }).kind === "string" &&
    typeof (value as { message: unknown }).message === "string"
  );
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (isCommandFailure(error)) {
      throw new BackendError(error.kind, error.message);
    }
    throw new BackendError("internal", error instanceof Error ? error.message : String(error));
  }
}

/** 画面から見たアプリ本体。Tauri コマンドを叩く薄い層で、契約は src/api/backend.ts と同じ。 */
export class TauriBackend implements Backend {
  async loadSettings(): Promise<AppSettings> {
    return invokeCommand<AppSettings>("load_settings");
  }

  async saveSettings(settings: AppSettings): Promise<void> {
    await invokeCommand<void>("save_settings", { settings });
  }

  async setApiKey(apiKey: string | null): Promise<void> {
    await invokeCommand<void>("set_api_key", { apiKey });
  }

  async hasApiKey(): Promise<boolean> {
    return invokeCommand<boolean>("has_api_key");
  }

  async listModels(llm?: LlmSettings): Promise<ModelInfo[]> {
    return invokeCommand<ModelInfo[]>("list_models", llm ? { llm } : undefined);
  }

  async listGenres(): Promise<GenrePreset[]> {
    return invokeCommand<GenrePreset[]>("list_genres");
  }

  async pickFolder(): Promise<string | null> {
    const selected = await open({ directory: true });
    if (Array.isArray(selected)) {
      return selected[0] ?? null;
    }
    return selected;
  }

  async createProject(folder: string, project: NewProject): Promise<ProjectOverview> {
    return invokeCommand<ProjectOverview>("create_project", { folder, project });
  }

  async openProject(folder: string): Promise<ProjectOverview> {
    return invokeCommand<ProjectOverview>("open_project", { folder });
  }

  async closeProject(): Promise<void> {
    await invokeCommand<void>("close_project");
  }

  async overview(): Promise<ProjectOverview> {
    return invokeCommand<ProjectOverview>("overview");
  }

  async pipeline(): Promise<PipelineStep[]> {
    return invokeCommand<PipelineStep[]>("pipeline");
  }

  async readFile(path: string): Promise<TextFile> {
    return invokeCommand<TextFile>("read_file", { path });
  }

  async writeFile(path: string, content: string, expectedHash: string | null): Promise<string> {
    return invokeCommand<string>("write_file", { path, content, expectedHash });
  }

  async textStats(text: string): Promise<TextStats> {
    return invokeCommand<TextStats>("text_stats", { text });
  }

  async parseRuby(text: string): Promise<Segment[]> {
    return invokeCommand<Segment[]>("parse_ruby", { text });
  }

  async analyzeQuality(text: string, targetChars: number | null): Promise<QualityReport> {
    return invokeCommand<QualityReport>("analyze_quality", { text, targetChars });
  }

  async generate(
    jobId: string,
    task: Task,
    onEvent: (event: GenerationEvent) => void,
  ): Promise<ChangeSet> {
    const onEventChannel = new Channel<GenerationEvent>();
    onEventChannel.onmessage = onEvent;
    return invokeCommand<ChangeSet>("generate", { jobId, task, onEvent: onEventChannel });
  }

  async cancelGeneration(jobId: string): Promise<void> {
    await invokeCommand<void>("cancel_generation", { jobId });
  }

  async applyChangeSet(changeSet: ChangeSet): Promise<ProjectOverview> {
    return invokeCommand<ProjectOverview>("apply_change_set", { changeSet });
  }
}
