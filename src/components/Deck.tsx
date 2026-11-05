// A screen's keys in the key deck at the bottom of the window. Rendered through a portal into the deck.

import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export function Deck({ children }: { children: ReactNode }) {
  const [el, setEl] = useState<HTMLElement | null>(null);
  useEffect(() => setEl(document.getElementById("deck-keys")), []);
  return el ? createPortal(children, el) : null;
}
