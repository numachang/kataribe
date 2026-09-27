// 本物のアプリを起動し、msedgedriver（WebDriver）でその画面を操作するセッションを作る。
//
// アプリは自分で起動して WebView2 のデバッグ用ポートを開き、msedgedriver をそのポートに接続させる。
// msedgedriver にアプリを起動させる方式（tauri-driver が使う方式）は、ポートなどの指定を環境変数
// （WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS）で渡す。管理者として動く CI では WebView2 がこの環境変数を
// 無視するため、CI ではコンピューター単位のポリシーで同じポートを渡す（.github/workflows/ci.yml）。

import { type ChildProcess, spawn } from "node:child_process";
import { createServer } from "node:net";
import path from "node:path";
import { Builder, type WebDriver } from "selenium-webdriver";

const DRIVER_PORT = 4444;
const STARTUP_TIMEOUT_MS = 30_000;
const STARTUP_POLL_MS = 200;

/** E2E で操作するアプリ（画面を埋め込んだデバッグ版）。環境変数で変えられる。 */
function appPath(): string {
  return path.resolve(process.env.KATARIBE_E2E_APP ?? "target/debug/kataribe.exe");
}

/**
 * WebView2 のデバッグ用ポート。CI ではポリシーに書いたポートと揃えるため環境変数で固定する。
 * 指定が無ければ空いているポートを選ぶ（9222 などは開発者の Chrome が使っていることがある）。
 */
async function debugPort(): Promise<number> {
  const fixed = process.env.KATARIBE_E2E_DEBUG_PORT;
  return fixed ? Number(fixed) : findFreePort();
}

function findFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

/** アプリの WebView2 と同じ版の msedgedriver。PATH に無ければ環境変数で場所を指定する。 */
function driverPath(): string {
  return process.env.KATARIBE_E2E_MSEDGEDRIVER ?? "msedgedriver.exe";
}

/** 操作中のアプリ。`close` でセッション・アプリ・msedgedriver をまとめて終わらせる。 */
export interface AppSession {
  app: WebDriver;
  close: () => Promise<void>;
}

/** アプリを `args` 付きで起動し、その画面を操作するセッションを作る。 */
export async function launchApp(args: string[]): Promise<AppSession> {
  const port = await debugPort();
  const driver = spawn(driverPath(), [`--port=${DRIVER_PORT}`], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  const appProcess = spawn(appPath(), args, {
    stdio: "ignore",
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
  });
  const stopProcesses = () => {
    appProcess.kill();
    driver.kill();
  };
  try {
    await waitUntilReady(driver, `http://127.0.0.1:${DRIVER_PORT}/status`, "msedgedriver");
    await waitUntilReady(
      appProcess,
      `http://127.0.0.1:${port}/json/version`,
      "アプリの WebView2（デバッグ用ポート）",
    );
    const app = await new Builder()
      .usingServer(`http://127.0.0.1:${DRIVER_PORT}`)
      .withCapabilities({
        browserName: "webview2",
        "ms:edgeOptions": { debuggerAddress: `127.0.0.1:${port}` },
      })
      .build();
    return {
      app,
      close: async () => {
        await app.quit().catch(() => {
          // 接続しただけのセッションなので、切れていても後始末は続ける
        });
        stopProcesses();
      },
    };
  } catch (error) {
    stopProcesses();
    throw error;
  }
}

/** `url` が応答するまで待つ。先に `process` が終了したら、その時点で失敗にする。 */
async function waitUntilReady(process: ChildProcess, url: string, name: string): Promise<void> {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (process.exitCode !== null) {
      throw new Error(`${name}が終了しました（終了コード ${process.exitCode}）。`);
    }
    if (await responds(url)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, STARTUP_POLL_MS));
  }
  throw new Error(`${name}が ${STARTUP_TIMEOUT_MS / 1000} 秒以内に応答しませんでした（${url}）。`);
}

async function responds(url: string): Promise<boolean> {
  try {
    return (await fetch(url)).ok;
  } catch {
    return false;
  }
}
