import { iconRects, type IconName } from "./icons";

type Size = 9 | 18 | 27 | 36 | 54;

/** A 9×9 pixel icon, drawn crisp at whole multiples of 9 px. */
export function Icon({ name, size = 18, className = "" }: { name: IconName; size?: Size; className?: string }) {
  const cls = size === 18 ? "" : ` s${size}`;
  return (
    <svg className={`i${cls} ${className}`.trim()} viewBox="0 0 9 9" aria-hidden="true" shapeRendering="crispEdges">
      {iconRects(name).map(([x, y, w]) => <rect key={`${x}.${y}`} x={x} y={y} width={w} height={1} />)}
    </svg>
  );
}
