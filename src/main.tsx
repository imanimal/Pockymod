import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/pocket-os.css";
import "./styles/app.css";
import "./styles/motion.css";
import "./styles/fx.css";
import "./styles/themes/standard.css";
import "./styles/themes/redline.css";
import { App } from "./App";
import { initTheme, loadInstalled } from "./lib/theme";
import { installDemo } from "./lib/demo";
import { installFx } from "./ui/fx";
import { installTips } from "./lib/keys";
import { installThemeEscape } from "./ui/themeEscape";

initTheme();
installDemo();
installFx();
installTips();
installThemeEscape();
void loadInstalled();

// Keys spring back when released: add .ack on release, drop it when the animation ends.
document.addEventListener("pointerup", (e) => {
  const key = (e.target as Element | null)?.closest?.(".btn, .nv, .wk, .chip, .row");
  if (!key || (key as HTMLButtonElement).disabled) return;
  key.classList.remove("ack");
  void (key as HTMLElement).offsetWidth;
  key.classList.add("ack");
  key.addEventListener("animationend", () => key.classList.remove("ack"), { once: true });
});

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
