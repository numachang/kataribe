// tauri-driver（WebDriver のサーバー）を起動し、アプリを操作するセッションを作る。

import { type ChildProcess, spawn } from "node:child_process";
import path from "node:path";
import { Builder, type WebDriver } from "selenium-webdriver";

const DRIVER_PORT = 4444;
const DRIVER_URL = `http://127.0.0.1:${DRIVER_PORT}`;
const STARTUP_TIMEOUT_MS = 10_000;
const STARTUP_POLL_MS = 200;

/** E2E に使うアプリと msedgedriver の場所。環境変数で変えられる。 */
function appPath(): string {
  return path.resolve(process.env.KATARIBE_E2E_APP ?? "target/debug/kataribe.exe");
}

function nativeDriverPath(): string {
  return process.env.KATARIBE_E2E_MSEDGEDRIVER ?? "msedgedriver.exe";
}

/** tauri-driver を起動し、接続を受け付けるまで待つ。 */
export async function startTauriDriver(): Promise<ChildProcess> {
  const driver = spawn("tauri-driver", ["--native-driver", nativeDriverPath()], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (driver.exitCode !== null) {
      throw new Error(`tauri-driver が終了しました（終了コード ${driver.exitCode}）。`);
    }
    if (await isDriverReady()) {
      return driver;
    }
    await new Promise((resolve) => setTimeout(resolve, STARTUP_POLL_MS));
  }
  driver.kill();
  throw new Error(
    "tauri-driver が起動しませんでした。PATH と msedgedriver の場所を確認してください。",
  );
}

async function isDriverReady(): Promise<boolean> {
  try {
    const response = await fetch(`${DRIVER_URL}/status`);
    return response.ok;
  } catch {
    return false;
  }
}

/** アプリを `args` 付きで起動し、その画面を操作するセッションを作る。 */
export function launchApp(args: string[]): Promise<WebDriver> {
  return new Builder()
    .usingServer(DRIVER_URL)
    .withCapabilities({
      browserName: "wry",
      "tauri:options": { application: appPath(), args },
    })
    .build();
}
