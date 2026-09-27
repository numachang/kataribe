import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { OverviewEntry, ProjectOverview } from "../../api/types";
import { useWorkspaceStore } from "../../store/workspaceStore";
import { ProjectTree } from "./ProjectTree";

afterEach(() => {
  useWorkspaceStore.getState().closeWorkspace();
});

function leaf(overrides: Partial<OverviewEntry>): OverviewEntry {
  return {
    path: null,
    label: "項目",
    kind: "other",
    exists: true,
    chars: 0,
    target_chars: null,
    error: null,
    children: [],
    ...overrides,
  };
}

describe("目次の文字数表示", () => {
  it("目標文字数が 0 のシーンは分母を出さない（8 / 0 字にならない）", () => {
    const overview: ProjectOverview = {
      root: "C:\\test",
      title: "テスト作品",
      target_length: 30000,
      total_chars: 8,
      sections: [
        {
          kind: "manuscript",
          label: "本文",
          entries: [
            leaf({
              path: "manuscript/01/s01.txt",
              label: "シーン",
              kind: "scene",
              chars: 8,
              target_chars: 0,
            }),
          ],
        },
      ],
    };
    useWorkspaceStore.getState().openWorkspace(overview);
    render(<ProjectTree />);

    expect(screen.getByText("8 字")).toBeInTheDocument();
    expect(screen.queryByText(/\/ 0 字/)).not.toBeInTheDocument();
  });

  it("作品全体の目標文字数が 0 のときも分母を出さない", () => {
    const overview: ProjectOverview = {
      root: "C:\\test",
      title: "テスト作品",
      target_length: 0,
      total_chars: 120,
      sections: [],
    };
    useWorkspaceStore.getState().openWorkspace(overview);
    render(<ProjectTree />);

    expect(screen.getByText("120 字")).toBeInTheDocument();
    expect(screen.queryByText(/\/ 0 字/)).not.toBeInTheDocument();
  });
});
