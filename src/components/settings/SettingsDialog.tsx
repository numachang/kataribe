import { type ReactNode, useEffect, useState } from "react";
import { BackendError } from "../../api/backend";
import { useBackend } from "../../api/context";
import type { AppSettings, DraftUnit, ProjectSettings } from "../../api/types";
import { flushIfOpen, writeBesideEditor } from "../../features/editor/openDocumentSync";
import { toErrorMessage } from "../../lib/errorMessage";
import {
  countProjectSettings,
  isSameProjectSettings,
  MANIFEST_PATH,
  withProjectSetting,
} from "../../lib/projectSettings";
import { useSettingsStore } from "../../store/settingsStore";
import { useUiStore } from "../../store/uiStore";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { Dialog } from "../Dialog";
import { AppSettingsSections } from "./AppSettingsSections";
import { ProjectSettingsSection } from "./ProjectSettingsSection";
import {
  GENERATION_NUMBER_KEYS,
  sanitizeGenerationNumber,
  sanitizeNumber,
} from "./settingsNumbers";
import "./SettingsDialog.css";

const EDITOR_DEFAULTS = {
  font_size: 17,
  line_height: 1.9,
} as const;

const APP_PANEL_ID = "settings-dialog-app";
const PROJECT_PANEL_ID = "settings-dialog-project";

/** 保存の直前に、数値欄の空欄・不正な入力を補正した設定を作る。 */
function sanitizeSettings(form: AppSettings): AppSettings {
  const generation = { ...form.generation };
  for (const key of GENERATION_NUMBER_KEYS) {
    generation[key] = sanitizeGenerationNumber(key, generation[key]);
  }
  return {
    ...form,
    generation,
    editor: {
      ...form.editor,
      font_size: sanitizeNumber(form.editor.font_size, 10, EDITOR_DEFAULTS.font_size),
      line_height: sanitizeNumber(form.editor.line_height, 1, EDITOR_DEFAULTS.line_height),
    },
  };
}

/** 作品の設定の、読み込んだ値・編集中の値と、読み込んだときの kataribe.yaml のハッシュ。 */
interface ProjectSettingsForm {
  stored: ProjectSettings;
  edited: ProjectSettings;
  hash: string;
}

/**
 * 保存する作品の設定。変わっていなければ null。
 * 数値は利用者が変えた項目だけを補正する（手で書かれた値を、触っていないのに書き換えないため）。
 */
function projectSettingsToSave(form: ProjectSettingsForm): ProjectSettings | null {
  let next = form.edited;
  for (const key of GENERATION_NUMBER_KEYS) {
    const value = next[key];
    if (value !== undefined && value !== form.stored[key]) {
      next = withProjectSetting(next, key, sanitizeGenerationNumber(key, value));
    }
  }
  return isSameProjectSettings(next, form.stored) ? null : next;
}

/** 工程の組み立てに使う生成単位（作品の設定があれば、そちらが優先）。 */
function effectiveDraftUnit(
  app: AppSettings | null,
  project: ProjectSettings | null,
): DraftUnit | undefined {
  return project?.draft_unit ?? app?.generation.draft_unit;
}

function projectSaveErrorMessage(error: unknown): string {
  if (error instanceof BackendError && error.kind === "conflict") {
    return "作品情報（kataribe.yaml）が、設定を開いたあとに変更されています。設定を開き直してから、もう一度変えてください。";
  }
  return toErrorMessage(error, "作品の設定を保存できませんでした。");
}

type Scope = "app" | "project";

interface SettingsDialogProps {
  onClose: () => void;
}

/**
 * 設定ダイアログ。LLM 接続・生成・エディタの見た目をまとめて変えられる。
 * 作品を開いているときは、その作品だけの設定（kataribe.yaml に保存）も変えられる。
 */
export function SettingsDialog({ onClose }: SettingsDialogProps) {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);
  const storedSettings = useSettingsStore((state) => state.settings);
  const projectTitle = useWorkspaceStore((state) => state.overview?.title ?? null);
  const isProjectOpen = projectTitle !== null;

  const [form, setForm] = useState<AppSettings | null>(storedSettings);
  const [scope, setScope] = useState<Scope>("app");
  const [projectForm, setProjectForm] = useState<ProjectSettingsForm | null>(null);
  const [projectLoadError, setProjectLoadError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (storedSettings && !form) {
      setForm(storedSettings);
    }
  }, [storedSettings, form]);

  useEffect(() => {
    if (!isProjectOpen) {
      return;
    }
    let isCurrent = true;
    // 作品情報をエディタで開いていれば、その編集を保存してから読む（ハッシュを最新の内容に合わせるため）
    flushIfOpen(backend, MANIFEST_PATH)
      .then(() => backend.loadProjectSettings())
      .then(({ settings, hash }) => {
        if (isCurrent) {
          setProjectForm({ stored: settings, edited: settings, hash });
        }
      })
      .catch((error: unknown) => {
        if (isCurrent) {
          setProjectLoadError(toErrorMessage(error, "作品の設定を読み込めませんでした。"));
        }
      });
    return () => {
      isCurrent = false;
    };
  }, [backend, isProjectOpen]);

  /** 生成単位が変わると工程の組み立てが変わりうるため、一覧を読み直す。失敗しても保存は済んでいる。 */
  async function refreshPipelineIfDraftUnitChanged(
    before: DraftUnit | undefined,
    after: DraftUnit | undefined,
  ): Promise<void> {
    if (before === after || !useWorkspaceStore.getState().overview) {
      return;
    }
    try {
      await useWorkspaceStore.getState().refreshPipeline(backend);
    } catch (error) {
      showToast(toErrorMessage(error, "工程の一覧を読み直せませんでした。"), "error");
    }
  }

  /** 作品の設定を保存する。保存できたら、保存した設定を返す。 */
  async function saveProjectSettings(
    current: ProjectSettingsForm,
    settings: ProjectSettings,
  ): Promise<ProjectSettings> {
    const hash = await writeBesideEditor(backend, {
      touches: (path) => path === MANIFEST_PATH,
      write: () => backend.saveProjectSettings(settings, current.hash),
      unsavedWorkMessage:
        "エディタで開いている作品情報（kataribe.yaml）に保存できていない編集があるため、作品の設定を保存しませんでした。先にその編集を保存してください。",
    });
    setProjectForm({ stored: settings, edited: settings, hash });
    return settings;
  }

  async function handleSave(): Promise<void> {
    if (!form) {
      return;
    }
    const sanitized = sanitizeSettings(form);
    const projectToSave = projectForm && projectSettingsToSave(projectForm);
    const draftUnitBefore = effectiveDraftUnit(storedSettings, projectForm?.stored ?? null);
    setIsSaving(true);
    try {
      try {
        await useSettingsStore.getState().save(backend, sanitized);
      } catch (error) {
        showToast(toErrorMessage(error, "設定を保存できませんでした。"), "error");
        return;
      }
      let savedProject = projectForm?.stored ?? null;
      if (projectForm && projectToSave) {
        try {
          savedProject = await saveProjectSettings(projectForm, projectToSave);
        } catch (error) {
          // アプリ全体の設定は保存できているので、そのぶんの反映はしてから、作品の設定の失敗を知らせる
          await refreshPipelineIfDraftUnitChanged(
            draftUnitBefore,
            effectiveDraftUnit(sanitized, savedProject),
          );
          showToast(
            `アプリ全体の設定は保存しましたが、作品の設定は保存できませんでした。${projectSaveErrorMessage(error)}`,
            "error",
          );
          return;
        }
      }
      await refreshPipelineIfDraftUnitChanged(
        draftUnitBefore,
        effectiveDraftUnit(sanitized, savedProject),
      );
      showToast("設定を保存しました。");
      onClose();
    } finally {
      setIsSaving(false);
    }
  }

  if (!form) {
    return (
      <Dialog title="設定" onClose={onClose}>
        <p>読み込んでいます…</p>
      </Dialog>
    );
  }

  const overriddenCount = projectForm ? countProjectSettings(projectForm.edited) : 0;

  return (
    <Dialog title="設定" onClose={onClose} wide>
      <div className="settings-dialog">
        {isProjectOpen && (
          <div className="settings-dialog__scopes" role="tablist" aria-label="設定する範囲">
            <ScopeTab
              panelId={APP_PANEL_ID}
              selected={scope === "app"}
              onSelect={() => setScope("app")}
            >
              アプリ全体
            </ScopeTab>
            <ScopeTab
              panelId={PROJECT_PANEL_ID}
              selected={scope === "project"}
              onSelect={() => setScope("project")}
            >
              この作品（{projectTitle}）
            </ScopeTab>
          </div>
        )}

        {/* 切り替えても入力中の値（API キーなど）が消えないよう、両方を描画したまま隠す */}
        <div
          id={APP_PANEL_ID}
          role={isProjectOpen ? "tabpanel" : undefined}
          className="settings-dialog__panel"
          hidden={scope !== "app"}
        >
          {overriddenCount > 0 && (
            <p className="settings-dialog__note">
              開いている作品には作品ごとの設定が {overriddenCount}{" "}
              項目あり、その作品ではそちらが優先されます。
            </p>
          )}
          <AppSettingsSections form={form} onChange={setForm} />
        </div>

        {isProjectOpen && (
          <div
            id={PROJECT_PANEL_ID}
            role="tabpanel"
            className="settings-dialog__panel"
            hidden={scope !== "project"}
          >
            <ProjectScope
              app={form}
              projectForm={projectForm}
              loadError={projectLoadError}
              onChange={(edited) => projectForm && setProjectForm({ ...projectForm, edited })}
            />
          </div>
        )}

        <div className="settings-dialog__actions">
          <button type="button" className="app-button" onClick={onClose}>
            キャンセル
          </button>
          <button
            type="button"
            className="app-button app-button--primary"
            disabled={isSaving}
            onClick={() => void handleSave()}
          >
            保存
          </button>
        </div>
      </div>
    </Dialog>
  );
}

interface ScopeTabProps {
  panelId: string;
  selected: boolean;
  onSelect: () => void;
  children: ReactNode;
}

function ScopeTab({ panelId, selected, onSelect, children }: ScopeTabProps) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      aria-controls={panelId}
      className={`settings-dialog__scope${selected ? " settings-dialog__scope--selected" : ""}`}
      onClick={onSelect}
    >
      {children}
    </button>
  );
}

interface ProjectScopeProps {
  app: AppSettings;
  projectForm: ProjectSettingsForm | null;
  loadError: string | null;
  onChange: (edited: ProjectSettings) => void;
}

/** 「この作品」を選んでいるときの中身。読み込みの途中や失敗も、ここで知らせる。 */
function ProjectScope({ app, projectForm, loadError, onChange }: ProjectScopeProps) {
  if (loadError) {
    return (
      <p className="settings-dialog__error" role="alert">
        {loadError}
      </p>
    );
  }
  if (!projectForm) {
    return <p>読み込んでいます…</p>;
  }
  return <ProjectSettingsSection app={app} value={projectForm.edited} onChange={onChange} />;
}
