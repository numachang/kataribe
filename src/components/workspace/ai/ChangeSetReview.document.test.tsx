import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Backend } from "../../../api/backend";
import { createMockBackend } from "../../../api/mock";
import type { ChangeSet, FileChange } from "../../../api/types";
import { useWorkspaceStore } from "../../../store/workspaceStore";
import { renderWithBackend } from "../../../test/renderWithBackend";
import { resetAllStores } from "../../../test/resetStores";
import { wrapBackend } from "../../../test/wrapBackend";
import { WorkspaceScreen } from "../WorkspaceScreen";

beforeEach(resetAllStores);
afterEach(resetAllStores);

const SUMMARY = "テスト用の変更案";

function characterFile(options: {
  role: string;
  summary?: string;
  body: string;
  order?: number;
}): string {
  return [
    "---",
    "name: 霧島 凛",
    "reading: きりしま りん",
    `role: ${options.role}`,
    `summary: ${options.summary ?? "盲目の少女探偵"}`,
    `order: ${options.order ?? 1}`,
    "---",
    options.body,
    "",
  ].join("\n");
}

interface SceneSpec {
  id: string;
  title: string;
  summary: string;
}

function chapterFile(scenes: SceneSpec[], storyline = "雨の夜の物語"): string {
  const sceneLines = scenes.flatMap((scene) => [
    `  - id: ${scene.id}`,
    `    title: ${scene.title}`,
    `    summary: ${scene.summary}`,
    "    pov: 霧島 凛",
    "    characters: [霧島 凛, 佐藤 健二]",
    "    place: 事務所",
    "    time: 雨の夜",
    "    target_chars: 2000",
  ]);
  return ["---", "title: 雨の匂い", "scenes:", ...sceneLines, "---", storyline, ""].join("\n");
}

function fileChange(path: string, content: string, previous: string | null): FileChange {
  return { kind: "write", path, content, previous, base_hash: null };
}

async function createEmptyProject(backend: Backend): Promise<void> {
  const overview = await backend.createProject("C:\\projects\\test-change-review", {
    title: "テスト作品",
    author: null,
    genre: "mystery",
    genre_note: null,
    rating: "general",
    target_length: 10000,
    idea: "雨の夜の探偵の物語",
  });
  useWorkspaceStore.getState().openWorkspace(overview);
  await useWorkspaceStore.getState().refreshPipeline(backend);
}

/** 生成を、決まった変更案を返すものに差し替え、AI パネルに変更案を出す。 */
async function showChangeSet(
  files: FileChange[],
  overrides: Partial<Backend> = {},
): Promise<ReturnType<typeof userEvent.setup>> {
  const user = userEvent.setup();
  const inner = createMockBackend({ delayMs: 0 });
  const changeSet: ChangeSet = { summary: SUMMARY, files, project_root: "C:\\projects\\test" };
  const backend = wrapBackend(inner, { generate: async () => changeSet, ...overrides });
  await createEmptyProject(backend);
  renderWithBackend(<WorkspaceScreen />, backend);
  await user.click(await screen.findByRole("button", { name: "次の工程を実行" }));
  await screen.findByText(SUMMARY);
  return user;
}

/** 項目名の行（項目名と値と印）。`container` の中から探す。 */
function fieldRow(label: string, container: HTMLElement = document.body): HTMLElement {
  const row = within(container).getByText(label).parentElement;
  if (row === null) {
    throw new Error(`項目「${label}」の行が見つかりません`);
  }
  return row;
}

function sceneSection(headingName: RegExp): HTMLElement {
  const section = screen.getByRole("heading", { name: headingName, level: 5 }).closest("section");
  if (section === null) {
    throw new Error("シーンのまとまりが見つかりません");
  }
  return section;
}

describe("人物資料の変更案", () => {
  const path = "characters/kirishima-rin.md";

  it("YAML のまま見せず、項目の一覧と本文に分けて見せる", async () => {
    await showChangeSet([
      fileChange(path, characterFile({ role: "主人公", body: "## 外見\n黒い髪の少女。" }), null),
    ]);

    expect(await screen.findByText("名前")).toBeInTheDocument();
    expect(screen.getByText("霧島 凛")).toBeInTheDocument();
    expect(screen.getByText("読み")).toBeInTheDocument();
    expect(screen.getByText("役割")).toBeInTheDocument();
    expect(screen.getByText("概要")).toBeInTheDocument();
    expect(screen.getByText("順番")).toBeInTheDocument();
    expect(screen.getByText("詳細")).toBeInTheDocument();
    expect(screen.getByText(/黒い髪の少女。/)).toBeInTheDocument();
    expect(screen.queryByText(/name:/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^---/)).not.toBeInTheDocument();
  });

  it("入力欄にはせず、読むだけにする", async () => {
    await showChangeSet([fileChange(path, characterFile({ role: "主人公", body: "本文" }), null)]);
    await screen.findByText("名前");

    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
  });

  it("変更前があれば、変わった項目にだけ印を付ける", async () => {
    await showChangeSet([
      fileChange(
        path,
        characterFile({ role: "探偵", body: "同じ本文" }),
        characterFile({ role: "主人公", body: "同じ本文" }),
      ),
    ]);
    await screen.findByText("名前");

    expect(within(fieldRow("役割")).getByText("変更")).toBeInTheDocument();
    expect(within(fieldRow("名前")).queryByText("変更")).not.toBeInTheDocument();
    expect(within(fieldRow("概要")).queryByText("変更")).not.toBeInTheDocument();
    expect(screen.getAllByText("変更")).toHaveLength(1);
  });

  it("本文が変わったときは、本文の見出しに印を付ける", async () => {
    await showChangeSet([
      fileChange(
        path,
        characterFile({ role: "主人公", body: "新しい本文" }),
        characterFile({ role: "主人公", body: "古い本文" }),
      ),
    ]);
    await screen.findByText("名前");

    expect(
      within(screen.getByRole("heading", { name: /詳細/ })).getByText("変更"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("変更")).toHaveLength(1);
  });

  it("新規のファイルには印を付けない", async () => {
    await showChangeSet([fileChange(path, characterFile({ role: "主人公", body: "本文" }), null)]);
    await screen.findByText("名前");

    expect(screen.queryByText("変更")).not.toBeInTheDocument();
  });

  it("「変更前を見る」で変更前の内容を同じ形で見せ、「変更後を見る」で戻る", async () => {
    const user = await showChangeSet([
      fileChange(
        path,
        characterFile({ role: "探偵", body: "新しい本文" }),
        characterFile({ role: "主人公", body: "古い本文" }),
      ),
    ]);
    await screen.findByText("名前");
    expect(screen.getByText("探偵")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "変更前を見る" }));

    expect(screen.getByText("主人公")).toBeInTheDocument();
    expect(screen.queryByText("探偵")).not.toBeInTheDocument();
    expect(screen.getByText(/古い本文/)).toBeInTheDocument();
    expect(screen.queryByText(/^---/)).not.toBeInTheDocument();
    // 変更前でも、変わった項目に印が付く
    expect(within(fieldRow("役割")).getByText("変更")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "変更後を見る" }));

    expect(screen.getByText("探偵")).toBeInTheDocument();
    expect(screen.getByText(/新しい本文/)).toBeInTheDocument();
  });

  it("front matter の無い内容は、理由を添えて書かれたまま見せる", async () => {
    await showChangeSet([fileChange(path, "名前も何もない本文です。", null)]);

    expect(await screen.findByText(/front matter がありません/)).toBeInTheDocument();
    expect(screen.getByText(/書かれたまま表示しています/)).toBeInTheDocument();
    expect(screen.getByText("名前も何もない本文です。")).toBeInTheDocument();
    expect(screen.queryByText("名前")).not.toBeInTheDocument();
  });
});

describe("章立ての変更案", () => {
  const path = "plot/chapters/01.md";
  const before: SceneSpec[] = [
    { id: "s01", title: "事務所に届いた依頼", summary: "雨の夜に依頼が届く" },
    { id: "s02", title: "現場へ", summary: "凛は現場へ向かう" },
    { id: "s04", title: "変わらない場面", summary: "ずっと同じ" },
  ];
  const after: SceneSpec[] = [
    { id: "s01", title: "事務所に届いた依頼", summary: "雨の夜、不審な依頼が届く" },
    { id: "s03", title: "新しい場面", summary: "追加された" },
    { id: "s04", title: "変わらない場面", summary: "ずっと同じ" },
  ];

  it("章題とシーンを、シーンごとのまとまりで見せる", async () => {
    await showChangeSet([fileChange(path, chapterFile(before), null)]);

    expect(await screen.findByText("章題")).toBeInTheDocument();
    expect(screen.getByText("雨の匂い")).toBeInTheDocument();
    const section = sceneSection(/^s01/);
    expect(within(section).getByText("タイトル")).toBeInTheDocument();
    expect(within(section).getByText("雨の夜に依頼が届く")).toBeInTheDocument();
    expect(within(section).getByText("視点")).toBeInTheDocument();
    expect(within(section).getByText("霧島 凛、佐藤 健二")).toBeInTheDocument();
    expect(within(section).getByText("場所")).toBeInTheDocument();
    expect(within(section).getByText("時間")).toBeInTheDocument();
    expect(within(section).getByText("目標文字数")).toBeInTheDocument();
    expect(within(section).getByText("2000")).toBeInTheDocument();
    expect(screen.getByText("ストーリーライン")).toBeInTheDocument();
    expect(screen.getByText("雨の夜の物語")).toBeInTheDocument();
    expect(screen.queryByText(/scenes:/)).not.toBeInTheDocument();
    expect(screen.queryByText("変更")).not.toBeInTheDocument();
  });

  it("シーンの変更・追加・削除が分かり、変わらないシーンには印が付かない", async () => {
    await showChangeSet([fileChange(path, chapterFile(after), chapterFile(before))]);
    await screen.findByText("章題");

    const changed = sceneSection(/^s01/);
    expect(within(changed).getAllByText("変更")).toHaveLength(2);
    expect(within(fieldRow("概要", changed)).getByText("変更")).toBeInTheDocument();
    expect(within(sceneSection(/^s03/)).getByText("追加")).toBeInTheDocument();
    expect(within(sceneSection(/^s02/)).getByText("削除")).toBeInTheDocument();
    const unchanged = sceneSection(/^s04/);
    expect(within(unchanged).queryByText("変更")).not.toBeInTheDocument();
    expect(within(unchanged).queryByText("追加")).not.toBeInTheDocument();
    expect(within(unchanged).queryByText("削除")).not.toBeInTheDocument();
  });

  it("変更前を見ると、変更後に無いシーンは「削除」、変更後にだけあるシーンは出ない", async () => {
    const user = await showChangeSet([fileChange(path, chapterFile(after), chapterFile(before))]);
    await screen.findByText("章題");

    await user.click(screen.getByRole("button", { name: "変更前を見る" }));

    expect(within(sceneSection(/^s02/)).getByText("削除")).toBeInTheDocument();
    expect(within(sceneSection(/^s01/)).getByText("雨の夜に依頼が届く")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^s03/ })).not.toBeInTheDocument();
  });

  it("章題とストーリーラインの変更にも印を付ける", async () => {
    await showChangeSet([
      fileChange(
        path,
        chapterFile(before, "新しいストーリーライン"),
        chapterFile(before, "古いストーリーライン").replace("title: 雨の匂い", "title: 霧の匂い"),
      ),
    ]);
    await screen.findByText("章題");

    expect(within(fieldRow("章題")).getByText("変更")).toBeInTheDocument();
    expect(
      within(screen.getByRole("heading", { name: /ストーリーライン/ })).getByText("変更"),
    ).toBeInTheDocument();
  });

  it("シーンの id が重複していて分けられないときは、理由と書かれたままの内容を見せる", async () => {
    const duplicated = chapterFile([
      { id: "s01", title: "一つ目", summary: "最初" },
      { id: "s01", title: "二つ目", summary: "id を直し忘れた" },
    ]);
    await showChangeSet([fileChange(path, duplicated, null)]);

    expect(await screen.findByText(/シーンの id「s01」が重複しているため/)).toBeInTheDocument();
    expect(screen.getByText(/id を直し忘れた/)).toBeInTheDocument();
    expect(screen.queryByText("章題")).not.toBeInTheDocument();
  });
});

describe("項目に分けない文書の変更案", () => {
  it("企画などは、今までどおり書かれたまま見せ、理由は出さない", async () => {
    const user = await showChangeSet([
      fileChange("concept.md", "# 企画\n---\nname: これは項目ではない\n---\n", "古い企画"),
    ]);

    expect(
      await screen.findByText(/name: これは項目ではない/, { exact: false }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/書かれたまま表示しています/)).not.toBeInTheDocument();
    expect(screen.queryByText("変更")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "変更前を見る" }));

    expect(screen.getByText("古い企画")).toBeInTheDocument();
  });
});

describe("項目に分けられないとき", () => {
  const path = "characters/kirishima-rin.md";
  const content = characterFile({ role: "主人公", body: "本文" });

  it("分けている間は、書かれたままの内容を見せ、分け終わると項目に切り替える", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const inner = createMockBackend({ delayMs: 0 });
    await showChangeSet([fileChange(path, content, null)], {
      parseDocument: async (parsePath, parseContent) => {
        await gate;
        return inner.parseDocument(parsePath, parseContent);
      },
    });

    // 分けるのを待っている間も、中身が見える
    expect(screen.getByText(/name: 霧島 凛/)).toBeInTheDocument();
    expect(screen.queryByText("名前")).not.toBeInTheDocument();

    release();

    expect(await screen.findByText("名前")).toBeInTheDocument();
    expect(screen.queryByText(/name: 霧島 凛/)).not.toBeInTheDocument();
  });

  it("分けるのに失敗しても、理由を添えて、書かれたままの内容を見せ続ける", async () => {
    await showChangeSet([fileChange(path, content, null)], {
      parseDocument: async () => {
        throw new Error("コマンドが登録されていません");
      },
    });

    expect(
      await screen.findByText(/項目に分けられませんでした（コマンドが登録されていません）/),
    ).toBeInTheDocument();
    expect(screen.getByText(/name: 霧島 凛/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "適用" })).toBeInTheDocument();
  });
});

describe("変更前と比べられないとき", () => {
  const path = "characters/kirishima-rin.md";
  const comparisonNote = "変更前と比べられないため、印は付けていません。";

  it("変更前が分けられない内容なら、印が無いのは変更が無いからではないと伝える", async () => {
    await showChangeSet([
      fileChange(
        path,
        characterFile({ role: "探偵", body: "本文" }),
        "front matter の無い古い内容",
      ),
    ]);

    expect(await screen.findByText("名前")).toBeInTheDocument();
    expect(screen.getByText(comparisonNote)).toBeInTheDocument();
    expect(screen.queryByText("変更")).not.toBeInTheDocument();
  });

  it("変更前が分けられて比べられたら、その一行は出さない", async () => {
    await showChangeSet([
      fileChange(
        path,
        characterFile({ role: "探偵", body: "本文" }),
        characterFile({ role: "主人公", body: "本文" }),
      ),
    ]);
    await screen.findByText("名前");

    expect(within(fieldRow("役割")).getByText("変更")).toBeInTheDocument();
    expect(screen.queryByText(/比べられないため/)).not.toBeInTheDocument();
  });

  it("新規のファイルには、その一行を出さない", async () => {
    await showChangeSet([fileChange(path, characterFile({ role: "探偵", body: "本文" }), null)]);
    await screen.findByText("名前");

    expect(screen.queryByText(/比べられないため/)).not.toBeInTheDocument();
  });

  it("変更前を分けている間は一行を出し、分け終わって比べられたら消える", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const inner = createMockBackend({ delayMs: 0 });
    const previous = characterFile({ role: "主人公", body: "本文" });
    await showChangeSet(
      [fileChange(path, characterFile({ role: "探偵", body: "本文" }), previous)],
      {
        parseDocument: async (parsePath, parseContent) => {
          if (parseContent === previous) {
            await gate;
          }
          return inner.parseDocument(parsePath, parseContent);
        },
      },
    );

    expect(await screen.findByText(comparisonNote)).toBeInTheDocument();

    release();

    await waitFor(() => {
      expect(screen.queryByText(comparisonNote)).not.toBeInTheDocument();
    });
    expect(within(fieldRow("役割")).getByText("変更")).toBeInTheDocument();
  });

  it("変更前を見ているときは「変更後と比べられない」と伝える", async () => {
    const user = await showChangeSet([
      fileChange(
        path,
        "front matter の無い新しい内容",
        characterFile({ role: "主人公", body: "本文" }),
      ),
    ]);
    await user.click(await screen.findByRole("button", { name: "変更前を見る" }));

    expect(await screen.findByText("名前")).toBeInTheDocument();
    expect(screen.getByText("変更後と比べられないため、印は付けていません。")).toBeInTheDocument();
  });
});

describe("アプリが知らない項目（利用者が足した項目）", () => {
  const path = "characters/kirishima-rin.md";
  const before = "変更前の内容";
  const after = "変更後の内容";

  /** 文字列ごとに決まった人物資料を返す分け方。知らない項目は meta の直下に載る。 */
  function parseWith(extraByContent: Record<string, Record<string, unknown>>): Partial<Backend> {
    return {
      parseDocument: async (_path, content) => ({
        document: {
          kind: "character",
          meta: { name: "霧島 凛", role: "主人公", summary: "探偵", ...extraByContent[content] },
          body: "本文",
        },
        parse_error: null,
      }),
    };
  }

  it("「その他の項目」として、キー名と値を出す", async () => {
    await showChangeSet(
      [fileChange(path, after, null)],
      parseWith({ [after]: { theme: "喪失", priority: 3 } }),
    );

    expect(await screen.findByText("その他の項目")).toBeInTheDocument();
    expect(screen.getByText("theme")).toBeInTheDocument();
    expect(screen.getByText("喪失")).toBeInTheDocument();
    expect(screen.getByText("priority")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("書き直しで落ちた項目と変わった項目に、印を付ける", async () => {
    await showChangeSet(
      [fileChange(path, after, before)],
      parseWith({ [before]: { theme: "喪失", note: "消えた" }, [after]: { theme: "再生" } }),
    );
    await screen.findByText("その他の項目");

    expect(within(fieldRow("theme")).getByText("変更")).toBeInTheDocument();
    expect(within(fieldRow("note")).getByText("変更")).toBeInTheDocument();
    expect(within(fieldRow("note")).getByText("（なし）")).toBeInTheDocument();
  });

  it("知らない項目が無ければ、「その他の項目」は出さない", async () => {
    await showChangeSet([fileChange(path, after, null)], parseWith({}));
    await screen.findByText("名前");

    expect(screen.queryByText("その他の項目")).not.toBeInTheDocument();
  });
});

describe("シーンの並び替え", () => {
  const path = "plot/chapters/01.md";

  it("並びだけが変わったシーンに、順序変更の印を付ける", async () => {
    const first: SceneSpec = { id: "s01", title: "一つ目", summary: "最初" };
    const second: SceneSpec = { id: "s02", title: "二つ目", summary: "次" };
    const third: SceneSpec = { id: "s03", title: "三つ目", summary: "最後" };
    await showChangeSet([
      fileChange(path, chapterFile([second, third, first]), chapterFile([first, second, third])),
    ]);
    await screen.findByText("章題");

    expect(within(sceneSection(/^s01/)).getByText("順序変更")).toBeInTheDocument();
    expect(screen.getAllByText("順序変更")).toHaveLength(1);
  });
});

describe("ゴミ箱へ移す変更案", () => {
  const trash: FileChange = {
    kind: "trash",
    path: "manuscript/01/s02.txt",
    files: [{ path: "manuscript/01/s02.txt", base_hash: "abcd1234", chars: 4210 }],
  };

  it("書き込みの変更と並べて、ゴミ箱へ移すファイルを文字数つきで見せる", async () => {
    await showChangeSet([fileChange("plot/chapters/01.md", chapterFile([]), null), trash]);

    const card = (
      await screen.findByText("manuscript/01/s02.txt", {
        selector: ".changeset-review__file-path",
      })
    ).closest(".changeset-review__file") as HTMLElement;
    expect(within(card).getByText("削除")).toBeInTheDocument();
    expect(within(card).getByText("ゴミ箱へ移すファイル")).toBeInTheDocument();
    expect(within(card).getByText("4,210 字")).toBeInTheDocument();
    // 書き込みの変更は、これまでどおり内容を見せる
    expect(screen.getByText("plot/chapters/01.md")).toBeInTheDocument();
  });

  it("内容を見せる欄や、変更前を見る切り替えは出さない", async () => {
    await showChangeSet([trash]);

    await screen.findByText("ゴミ箱へ移すファイル");
    expect(screen.queryByRole("button", { name: "変更前を見る" })).not.toBeInTheDocument();
  });

  it("テキストとして読めず、ハッシュの無いファイルは、移せないことを添える", async () => {
    await showChangeSet([
      {
        kind: "trash",
        path: "manuscript/01/s03.txt",
        files: [{ path: "manuscript/01/s03.txt", base_hash: null, chars: 0 }],
      },
    ]);

    expect(await screen.findByText("テキストとして読めないため、移せません")).toBeInTheDocument();
  });
});
