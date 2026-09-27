import { defineConfig } from "vitest/config";

// 本物のアプリを操作する E2E テスト（pnpm e2e）。画面の単体テスト（pnpm test）とは別に動かす。
// テストは 1 つのアプリを順に操作するので、並列にしない。
export default defineConfig({
  test: {
    environment: "node",
    globals: true,
    include: ["e2e/**/*.e2e.ts"],
    fileParallelism: false,
    sequence: { concurrent: false },
    testTimeout: 30_000,
    hookTimeout: 60_000,
  },
});
