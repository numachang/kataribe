import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { createBackend } from "./api";
import { BackendProvider } from "./api/context";
import "./styles/global.css";

const container = document.getElementById("root");
if (container) {
  createRoot(container).render(
    <StrictMode>
      <BackendProvider backend={createBackend()}>
        <App />
      </BackendProvider>
    </StrictMode>,
  );
}
