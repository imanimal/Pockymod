import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
// Fonts are bundled with the app, never loaded from the network.
import "@fontsource/space-grotesk/400.css";
import "@fontsource/space-grotesk/500.css";
import "@fontsource/space-grotesk/600.css";
import "@fontsource/space-grotesk/700.css";
import "@fontsource/space-mono/400.css";
import "@fontsource/space-mono/700.css";
import "@fontsource/doto/800.css";
import "@fontsource/doto/900.css";
import "./styles/pocket-os.css";
import "./styles/app.css";
import { Shell } from "./App";
import { inTauri } from "./lib/api";
import { AppProvider, type View } from "./lib/store";

if (inTauri) document.documentElement.dataset.frame = "own";
const params = new URLSearchParams(window.location.search);
const initial = (params.get("view") as View | null) ?? "browse";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AppProvider initialView={initial}>
      <Shell />
    </AppProvider>
  </StrictMode>,
);
