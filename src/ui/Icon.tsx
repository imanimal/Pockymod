import { useSyncExternalStore } from "react";
import { iconRects, type IconName } from "./icons";
import { LINE_ICONS } from "./icons-line";
import { subscribeTheme, themeState } from "../lib/theme";

type Size = 9 | 18 | 27 | 36 | 54;
const iconStyle = () => themeState().icons;

/** A 9×9 pixel icon, drawn crisp at whole multiples of 9 px; or its line version when the theme asks for line icons. */
export function Icon({ name, size = 18, className = "" }: { name: IconName; size?: Size; className?: string }) {
  const style = useSyncExternalStore(subscribeTheme, iconStyle);
  const cls = size === 18 ? "" : ` s${size}`;
  const line = style === "line" ? LINE_ICONS[name] : undefined;
  if (line) {
    return (
      <svg className={`i ln${cls} ${className}`.trim()} viewBox="0 0 24 24" aria-hidden="true">
        <path d={line} vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }
  return (
    <svg className={`i${cls} ${className}`.trim()} viewBox="0 0 9 9" aria-hidden="true" shapeRendering="crispEdges">
      {iconRects(name).map(([x, y, w]) => <rect key={`${x}.${y}`} x={x} y={y} width={w} height={1} />)}
    </svg>
  );
}
