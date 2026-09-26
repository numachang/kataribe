import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import type { Backend } from "../api/backend";
import { BackendProvider } from "../api/context";

/** BackendProvider で包んで render する。個々のテストでは偽バックエンドを渡す。 */
export function renderWithBackend(ui: ReactElement, backend: Backend) {
  return render(<BackendProvider backend={backend}>{ui}</BackendProvider>);
}
