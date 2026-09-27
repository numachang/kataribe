import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAutosaveScheduler } from "./autosave";

describe("createAutosaveScheduler", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("入力が止まって指定時間が経つと保存する", () => {
    const onDue = vi.fn();
    const scheduler = createAutosaveScheduler(1000, onDue);

    scheduler.notifyChange();
    vi.advanceTimersByTime(999);
    expect(onDue).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onDue).toHaveBeenCalledTimes(1);
  });

  it("待っている間に再度入力があると、時間を数え直す", () => {
    const onDue = vi.fn();
    const scheduler = createAutosaveScheduler(1000, onDue);

    scheduler.notifyChange();
    vi.advanceTimersByTime(700);
    scheduler.notifyChange();
    vi.advanceTimersByTime(700);
    expect(onDue).not.toHaveBeenCalled();

    vi.advanceTimersByTime(300);
    expect(onDue).toHaveBeenCalledTimes(1);
  });

  it("flushIfPending は待たずにすぐ保存する（Ctrl+S 相当）", () => {
    const onDue = vi.fn();
    const scheduler = createAutosaveScheduler(1000, onDue);

    scheduler.notifyChange();
    scheduler.flushIfPending();
    expect(onDue).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(1000);
    expect(onDue).toHaveBeenCalledTimes(1);
  });

  it("保留中の変更がなければ flushIfPending は何もしない", () => {
    const onDue = vi.fn();
    const scheduler = createAutosaveScheduler(1000, onDue);

    scheduler.flushIfPending();
    expect(onDue).not.toHaveBeenCalled();
  });

  it("cancel すると保留中のタイマーは実行されない", () => {
    const onDue = vi.fn();
    const scheduler = createAutosaveScheduler(1000, onDue);

    scheduler.notifyChange();
    scheduler.cancel();
    vi.advanceTimersByTime(1000);
    expect(onDue).not.toHaveBeenCalled();
  });

  it("isPending は保留状態を反映する", () => {
    const scheduler = createAutosaveScheduler(1000, () => {});
    expect(scheduler.isPending).toBe(false);
    scheduler.notifyChange();
    expect(scheduler.isPending).toBe(true);
    vi.advanceTimersByTime(1000);
    expect(scheduler.isPending).toBe(false);
  });
});
