// Sharing to the marketplace (a checked package, a saved file, a pre-filled GitHub issue) and "Make your own".
import { useMemo, useState } from "react";
import { Dialog } from "../../ui/Dialog";
import { Icon } from "../../ui/Icon";
import { Key } from "../../ui/pocket";
import { toast } from "../../ui/toast";
import { errorText, openExternal, saveTextAs } from "../../lib/api";
import { validateExtension, validateTheme } from "../../lib/addon-rules.mjs";
import { DOCS, REPO, packageFileName, submissionUrl } from "../../lib/market";

const open = (url: string) => openExternal(url).catch((e) => {
  // the browser demo has no desktop browser to hand the page to
  if (!("__TAURI_INTERNALS__" in window)) { window.open(url, "_blank", "noopener,noreferrer"); return; }
  toast(errorText(e), true);
});
export const openDocs = (which: keyof typeof DOCS) => void open(DOCS[which]);

/** Share a theme or extension: check it, save the file, open the submission form, and say what happens next. */
export function ShareDialog({ kind, text, onClose }: { kind: "theme" | "extension"; text: string; onClose: () => void }) {
  const parsed = useMemo(() => { try { return JSON.parse(text) as { id?: string; name?: string }; } catch { return null; } }, [text]);
  const check = useMemo(() => (parsed ? (kind === "theme" ? validateTheme(parsed) : validateExtension(parsed)) : { ok: false, errors: ["Not valid JSON"], warnings: [] }), [parsed, kind]);
  const [saved, setSaved] = useState(false);
  const [opened, setOpened] = useState(false);
  const name = parsed?.name ?? "My add-on";
  const sub = useMemo(() => submissionUrl(kind, name, text), [kind, name, text]);

  const save = async () => { try { if (await saveTextAs(packageFileName(kind, parsed?.id ?? "addon"), text, ["json"])) setSaved(true); } catch (e) { toast(errorText(e), true); } };
  const copy = () => navigator.clipboard.writeText(text).then(() => toast("Copied")).catch(() => toast("Couldn't copy", true));

  return (
    <Dialog icon="share" title="Share to the marketplace" count={name} onClose={onClose}>
      {!check.ok ? (
        <>
          <div className="note bad">Fix these before sharing</div>
          <ul className="ext-errs">{check.errors.slice(0, 8).map((e) => <li key={e} className="mono">{e}</li>)}</ul>
          <p className="dim">Then install it from file again and share</p>
        </>
      ) : (
        <>
          <p>Submissions are GitHub issues. Needs a GitHub account</p>
          <ol className="ext-steps">
            <li className={saved ? "done" : ""}>
              <b>Save the file</b> <span className="dim">Keep it for updates</span>
              <div className="actions"><Key size="sm" icon="file" onClick={save}>{saved ? "Saved" : "Save the file"}</Key><Key size="sm" variant="ghost" onClick={copy}>Copy the JSON</Key></div>
            </li>
            <li className={opened ? "done" : ""}>
              <b>Open the form</b>
              <div className="actions"><Key size="sm" icon="web" onClick={() => { void open(sub.url); setOpened(true); }}>Open the form on GitHub</Key></div>
            </li>
            <li>
              <b>{sub.withPackage ? "Add tags and a description" : "Drag the file into the Package box"}</b>
            </li>
            <li><b>Submit</b> <span className="dim">Shows here within an hour of approval</span></li>
          </ol>
          {check.warnings.length > 0 && <div className="note">{check.warnings.slice(0, 3).map((w) => <div key={w}>{w}</div>)}</div>}
          <p className="dim" style={{ fontSize: ".86em" }}>Or open a pull request adding it to <code>marketplace/{kind === "theme" ? "themes" : "extensions"}/</code> and <code>marketplace/index.json</code>{" "}
            <button type="button" className="linkish" onClick={() => openDocs("marketplace")}>How sharing works</button></p>
        </>
      )}
      <div className="actions" style={{ justifyContent: "flex-end" }}><Key variant="ghost" onClick={onClose}>Close</Key></div>
    </Dialog>
  );
}

/** The current look, read from the page, as a theme to start from. */
function currentLookTheme(): string {
  const cs = getComputedStyle(document.documentElement);
  const keys = ["--bg", "--bg2", "--surface", "--surface-2", "--line", "--text", "--text-2", "--text-3", "--accent", "--on-accent", "--chrome-bg", "--chrome-fg",
    "--disp-bg", "--disp-fg", "--disp-hot", "--disp-bad", "--ok", "--warn", "--bad", "--acc2"];
  const scheme = document.documentElement.dataset.scheme === "dark" ? "dark" : "light";
  const tokens = Object.fromEntries(keys.map((k) => [k, cs.getPropertyValue(k).trim()]).filter(([, v]) => v));
  return JSON.stringify({ format: "pocket-mods.theme", version: 1, id: "my-theme", name: "My theme", release: "1.0.0", author: "You", description: "Starts from my current look", scheme, [scheme]: tokens }, null, 2);
}

const STARTER = {
  format: "pocket-mods.extension", version: 1, id: "my-extension", name: "My extension", release: "1.0.0", author: "You",
  description: "A shelf of mods and a launch option, to start from.",
  contributes: {
    shelves: [{ id: "photo", title: "Photo mode", hint: "Poses and cameras", query: { tags: ["Poses"], sort: "endorsements" } }],
    launchOptions: [{ id: "skip", name: "Skip the start screen", args: ["-skipStartScreen"] }],
    links: [{ id: "wiki", title: "Cyberpunk modding wiki", url: "https://wiki.redmodding.org/cyberpunk-2077-modding" }],
  },
};

export function MakeDialog({ onClose }: { onClose: () => void }) {
  const save = (name: string, text: string) => saveTextAs(name, text, ["json"]).then((ok) => { if (ok) toast(`Saved ${name}`); }).catch((e) => toast(errorText(e), true));
  return (
    <Dialog icon="plus" title="Make your own" onClose={onClose}>
      <div className="grid2 ext-make">
        <div className="ext-make-k">
          <div className="ext-sec-h"><Icon name="sun" size={9} /> A theme</div>
          <ol className="ext-steps compact">
            <li>Save your current look as a starter</li>
            <li>Change the colours. <code>dark</code> and <code>light</code> each take a set</li>
            <li>Set your own <code>id</code> and <code>name</code></li>
            <li>Install from file to try it</li>
          </ol>
          <div className="actions"><Key size="sm" icon="file" onClick={() => void save("my-theme.pmtheme.json", currentLookTheme())}>Save a starter theme</Key>
            <Key size="sm" variant="ghost" icon="web" onClick={() => openDocs("themes")}>Theme guide</Key></div>
        </div>
        <div className="ext-make-k">
          <div className="ext-sec-h"><Icon name="addons" size={9} /> An extension</div>
          <ol className="ext-steps compact">
            <li>Save the starter</li>
            <li>Add shelves, rules, tools, commands, links or a panel</li>
            <li>Bump <code>release</code> for each version</li>
            <li>Install from file to try it</li>
          </ol>
          <div className="actions"><Key size="sm" icon="file" onClick={() => void save("my-extension.pmext.json", JSON.stringify(STARTER, null, 2))}>Save a starter extension</Key>
            <Key size="sm" variant="ghost" icon="web" onClick={() => openDocs("extensions")}>Extension guide</Key></div>
        </div>
      </div>
      <div className="note">Share it from Installed <button type="button" className="linkish" onClick={() => openDocs("marketplace")}>How the marketplace works</button>
        {" · "}<button type="button" className="linkish" onClick={() => void open(`${REPO}/tree/main/marketplace`)}>Examples on GitHub</button></div>
      <div className="actions" style={{ justifyContent: "flex-end" }}><Key variant="ghost" onClick={onClose}>Close</Key></div>
    </Dialog>
  );
}
