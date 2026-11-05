// Renders a Nexus BBCode description as React elements (never raw HTML). Links don't leave the app: they're
// shown with their address, and Nexus links open in the in-app Nexus window.

import type { ReactNode } from "react";
import { parse, type Node } from "../lib/bbcode";

function render(nodes: Node[], onLink?: (href: string) => void, key = "n"): ReactNode[] {
  return nodes.map((n, i) => {
    const k = `${key}-${i}`;
    switch (n.t) {
      case "text":
        return n.v;
      case "br":
        return <br key={k} />;
      case "hr":
        return <hr key={k} />;
      case "b":
        return <b key={k}>{render(n.c, onLink, k)}</b>;
      case "i":
        return <i key={k}>{render(n.c, onLink, k)}</i>;
      case "u":
        return <u key={k}>{render(n.c, onLink, k)}</u>;
      case "s":
        return <s key={k}>{render(n.c, onLink, k)}</s>;
      case "center":
        return (
          <div key={k} className="d-center">
            {render(n.c, onLink, k)}
          </div>
        );
      case "quote":
        return <blockquote key={k}>{render(n.c, onLink, k)}</blockquote>;
      case "code":
        return <code key={k}>{render(n.c, onLink, k)}</code>;
      case "spoiler":
        return (
          <details key={k}>
            <summary>Spoiler</summary>
            {render(n.c, onLink, k)}
          </details>
        );
      case "h":
        return <h4 key={k}>{render(n.c, onLink, k)}</h4>;
      case "p":
        return <span key={k}>{render(n.c, onLink, k)}</span>;
      case "list": {
        const items = n.items.map((it, j) => <li key={j}>{render(it, onLink, `${k}-${j}`)}</li>);
        return n.ordered ? <ol key={k}>{items}</ol> : <ul key={k}>{items}</ul>;
      }
      case "link":
        return (
          <a
            key={k}
            href={n.href}
            title={n.href}
            onClick={(e) => {
              e.preventDefault();
              onLink?.(n.href);
            }}
          >
            {render(n.c, onLink, k)}
          </a>
        );
      case "img":
        return (
          <span key={k} className="d-img" title={n.src}>
            [image]
          </span>
        );
    }
  });
}

export function Description({ source, onLink }: { source: string; onLink?: (href: string) => void }) {
  return <div className="desc">{render(parse(source), onLink)}</div>;
}
