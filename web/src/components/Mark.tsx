/**
 * The kuro mark: 黒 (kuro, black) drawn as linear memory. Stripped to the stem, its base and the one cell just
 * written in vermilion; the four dots are the meter's 2-on / 2-off segments. One ink path, one square, every edge on a
 * whole pixel. Identical to public/favicon.svg; only the canvas differs (cropped to the 14x14 ink here, so its box is
 * the mark; on the 16x16 carbon tile there).
 */
export function Mark() {
  return (
    <svg className="mark-logo" viewBox="1 1 14 14" width="14" height="14" shapeRendering="crispEdges" aria-hidden="true" focusable="false">
      <path d="M7 1h2v10H7zM3 10h10v1H3zM1 13h2v2H1zM5 13h2v2H5zM9 13h2v2H9zM13 13h2v2h-2z" style={{ fill: "var(--ink)" }} />
      <rect x="9" y="2" width="4" height="2" style={{ fill: "var(--signal)" }} />
    </svg>
  );
}
