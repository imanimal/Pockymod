import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/pocket-os.css";
import "./styles/app.css";
import { App } from "./App";
import { applyTheme } from "./lib/theme";
import { installDemo } from "./lib/demo";

applyTheme();
installDemo();

// Keys spring back when released: add .ack on release, drop it when the animation ends.
document.addEventListener("pointerup", (e) => {
  const key = (e.target as Element | null)?.closest?.(".btn, .nv, .wk, .chip");
  if (!key || (key as HTMLButtonElement).disabled) return;
  key.classList.remove("ack");
  void (key as HTMLElement).offsetWidth;
  key.classList.add("ack");
  key.addEventListener("animationend", () => key.classList.remove("ack"), { once: true });
});

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
