import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ActionMenu } from "./ActionMenu";

function renderMenu(disabledReason?: string) {
  const onSelect = vi.fn();
  const onRename = vi.fn();
  render(
    <div>
      <button type="button">外側のボタン</button>
      <ActionMenu
        label="「霧島 凛」の操作"
        items={[
          { label: "削除", onSelect },
          { label: "名前を変える", onSelect: onRename, disabledReason },
        ]}
      />
    </div>,
  );
  return { onSelect, onRename };
}

const TRIGGER = "「霧島 凛」の操作";

describe("ActionMenu", () => {
  it("開くまでは項目を出さず、開くと項目が並ぶ", async () => {
    const user = userEvent.setup();
    renderMenu();
    expect(screen.queryByRole("menuitem")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: TRIGGER }));

    expect(screen.getByRole("button", { name: TRIGGER })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "削除",
      "名前を変える",
    ]);
  });

  it("項目を選ぶと、その操作を呼んでメニューを閉じる", async () => {
    const user = userEvent.setup();
    const { onSelect } = renderMenu();
    await user.click(screen.getByRole("button", { name: TRIGGER }));

    await user.click(screen.getByRole("menuitem", { name: "削除" }));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("Escape で閉じて、開くボタンにフォーカスを戻す", async () => {
    const user = userEvent.setup();
    renderMenu();
    await user.click(screen.getByRole("button", { name: TRIGGER }));

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: TRIGGER })).toHaveFocus();
  });

  it("メニューの外をクリックすると閉じる", async () => {
    const user = userEvent.setup();
    renderMenu();
    await user.click(screen.getByRole("button", { name: TRIGGER }));

    await user.click(screen.getByRole("button", { name: "外側のボタン" }));

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("フォーカスがメニューの外へ移ると閉じる", async () => {
    const user = userEvent.setup();
    renderMenu();
    await user.click(screen.getByRole("button", { name: TRIGGER }));

    act(() => screen.getByRole("button", { name: "外側のボタン" }).focus());

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("開くと最初の項目にフォーカスが移り、矢印キーで項目を移れる", async () => {
    const user = userEvent.setup();
    renderMenu();
    await user.click(screen.getByRole("button", { name: TRIGGER }));
    expect(screen.getByRole("menuitem", { name: "削除" })).toHaveFocus();

    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "名前を変える" })).toHaveFocus();

    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "削除" })).toHaveFocus();

    await user.keyboard("{ArrowUp}");
    expect(screen.getByRole("menuitem", { name: "名前を変える" })).toHaveFocus();
  });

  it("理由を添えた項目は無効になり、理由を title に出して、選べない", async () => {
    const user = userEvent.setup();
    const { onRename } = renderMenu("生成している間は操作できません。");
    await user.click(screen.getByRole("button", { name: TRIGGER }));

    const disabledItem = screen.getByRole("menuitem", { name: "名前を変える" });
    expect(disabledItem).toBeDisabled();
    expect(disabledItem).toHaveAttribute("title", "生成している間は操作できません。");
    await user.click(disabledItem);
    expect(onRename).not.toHaveBeenCalled();
  });
});
