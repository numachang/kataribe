import { beforeEach, describe, expect, it } from "vitest";
import type { AppSettings } from "../api/types";
import { createStubBackend } from "../test/fakeBackend";
import { useSettingsStore } from "./settingsStore";

const SAMPLE_SETTINGS: AppSettings = {
  llm: { base_url: "http://localhost:1234/v1", model: "test-model" },
  generation: {
    draft_unit: "scene",
    chars_per_call: 1500,
    context_tokens: 16384,
    temperature: 0.8,
    polish: true,
    quality_retries: 1,
    disable_thinking: true,
  },
  editor: { vertical: true, font_style: "mincho", font_size: 17, line_height: 1.9 },
  recent_projects: [],
};

function reset(): void {
  useSettingsStore.setState({ settings: null, status: "idle", errorMessage: null });
}

describe("useSettingsStore", () => {
  beforeEach(reset);

  it("load は Backend から設定を取得して保持する", async () => {
    const backend = createStubBackend({ loadSettings: async () => SAMPLE_SETTINGS });

    await useSettingsStore.getState().load(backend);

    expect(useSettingsStore.getState().settings).toEqual(SAMPLE_SETTINGS);
    expect(useSettingsStore.getState().status).toBe("ready");
  });

  it("load が失敗すると status が error になる", async () => {
    const backend = createStubBackend({
      loadSettings: async () => {
        throw new Error("接続できません");
      },
    });

    await useSettingsStore.getState().load(backend);

    expect(useSettingsStore.getState().status).toBe("error");
    expect(useSettingsStore.getState().errorMessage).toBe("接続できません");
  });

  it("save は Backend へ保存し、保持している設定も更新する", async () => {
    const saved: AppSettings[] = [];
    const backend = createStubBackend({
      saveSettings: async (settings) => {
        saved.push(settings);
      },
    });

    const next: AppSettings = {
      ...SAMPLE_SETTINGS,
      llm: { ...SAMPLE_SETTINGS.llm, model: "別のモデル" },
    };
    await useSettingsStore.getState().save(backend, next);

    expect(saved).toEqual([next]);
    expect(useSettingsStore.getState().settings).toEqual(next);
  });
});
