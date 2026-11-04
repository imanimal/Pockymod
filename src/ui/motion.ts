// Small helpers for the one-shot motion classes in pocket-os.css (.boot, .nope, .saved, .gone …).
import { useEffect, useRef, type RefObject } from "react";

/** Add a class until its animation ends (or a moment passes, when motion is off and nothing animates). */
export function oneShot(el: Element | null | undefined, cls: string) {
  if (!el) return;
  el.classList.remove(cls);
  void (el as HTMLElement).offsetWidth; // restart it
  el.classList.add(cls);
  const off = () => el.classList.remove(cls);
  el.addEventListener("animationend", (e) => { if (e.target === el) off(); }, { once: true });
  setTimeout(off, 1500);
}

/** A refused action: a two-pixel shake. */
export const nope = (el: Element | null | undefined) => oneShot(el, "nope");
/** Something was stored: a green ring that fades. */
export const saved = (el: Element | null | undefined) => oneShot(el, "saved");

/** Play `cls` on the element each time `key` changes, the first render included. */
export function useOneShot<T extends Element>(cls: string, key: unknown): RefObject<T | null> {
  const ref = useRef<T>(null);
  useEffect(() => { oneShot(ref.current, cls); }, [cls, key]);
  return ref;
}

/** Play an exit (.gone) on an element, then run `done`. */
export function leave(el: Element | null | undefined, done: () => void) {
  if (!el || el.classList.contains("gone")) { done(); return; }
  el.classList.add("gone");
  let ran = false;
  const finish = () => { if (!ran) { ran = true; done(); } };
  el.addEventListener("animationend", finish, { once: true });
  setTimeout(finish, 320);
}
