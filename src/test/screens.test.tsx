// Renders every screen against the browser-preview mock and checks the Pocket rules a test can check:
// at most one orange key or band per screen (rule 1), and dot-matrix numbers only inside LCDs (rule 2).

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { Shell } from "../App";
import { _setKey, handle } from "../lib/mock";
import { AppProvider, type View } from "../lib/store";
import "../styles/pocket-os.css";

const SCREENS: View[] = ["browse", "library", "collections", "downloads", "loadorder", "health", "settings"];

beforeAll(() => _setKey(false));
afterEach(cleanup);

async function renderView(v: View) {
  const r = render(
    <AppProvider initialView={v}>
      <Shell />
    </AppProvider>,
  );
  // let the mock's delayed answers arrive
  await act(async () => {
    await new Promise((res) => setTimeout(res, 900));
  });
  return r;
}

describe("screens", () => {
  for (const v of SCREENS) {
    it(`${v}: renders with at most one orange key or band`, async () => {
      const { container } = await renderView(v);
      await waitFor(() => expect(container.querySelector(".page")!.children.length).toBeGreaterThan(0));
      const orange = container.querySelectorAll(".btn.primary, .ph.hot");
      expect(orange.length, [...orange].map((o) => o.textContent).join(" | ")).toBeLessThanOrEqual(1);
      // Doto digits only inside LCDs: no .d-num outside a .display
      for (const n of container.querySelectorAll(".d-num")) expect(n.closest(".display")).not.toBeNull();
    });
  }

  it("the gallery shows every component", async () => {
    const { container } = await renderView("gallery");
    for (const sel of [".btn.primary", ".btn.dark", ".btn.ghost", ".btn.sm", ".btn.lg", ".nv.on", ".nv .led.on", ".nv .pg", ".nv .ct", ".ph.hot", ".ph.bad", ".display.hot", ".display.bad", ".display.big", ".display .meter", ".fbox", ".chip.on", ".seg", ".sw", ".duepill", ".cchip", ".row", ".grp", ".winkeys", ".deck", ".m-empty"]) {
      expect(container.querySelector(sel), sel).not.toBeNull();
    }
  });
});

describe("mock queue (free account)", () => {
  it("queues a mod with its missing frameworks and waits for a click", async () => {
    const ids = (await handle("queue_mod", { modId: 90004, fileId: null })) as number[];
    // ArchiveXL needs RED4ext; RED4ext is installed in the mock, ArchiveXL is not
    expect(ids.length).toBe(2);
    const v = (await handle("queue_view")) as { summary: { needsClick: number; waitingOnUser: boolean } };
    expect(v.summary.needsClick).toBe(2);
    expect(v.summary.waitingOnUser).toBe(true);
  });
});
