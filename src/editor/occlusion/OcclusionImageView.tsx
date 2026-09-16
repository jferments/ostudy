import { useId } from "react";

import type { NormalizedRect } from "../../ipc/questions";
import { Button } from "../../ui/primitives/Button";
import { toScreenPoint, type Size } from "./geometry";
import { useOcclusionWorkspace } from "./useOcclusionWorkspace";

interface OcclusionImageViewProps {
  imageUrl: string;
  imageKey: string;
  imageSize: Size;
  targets: readonly NormalizedRect[];
  label: string;
}
const noSelection = () => undefined;

/** A navigation-only view. Practice supplies raster bytes from its authorized native
 * renderer, never a source-asset URL with overlay-only masking.
 */
export function OcclusionImageView({ imageUrl, imageKey, imageSize, targets, label }: OcclusionImageViewProps) {
  const helpId = useId();
  const viewport = useOcclusionWorkspace({
    imageKey, imageSize, masks: [], selectedMaskId: null, navigationOnly: true,
    onSelect: noSelection, onMutation: noSelection, onDelete: noSelection,
    onUndo: noSelection, onRedo: noSelection,
  });
  const { frame, transform } = viewport;
  return (
    <section className="tw:grid tw:min-w-0 tw:gap-3" aria-label={label}>
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2" role="group" aria-label="Study image view controls">
        <Button disabled={viewport.viewport.zoom <= 1} onClick={() => viewport.zoom(1 / 1.5)}>Zoom out</Button>
        <Button disabled={viewport.viewport.zoom >= 32} onClick={() => viewport.zoom(1.5)}>Zoom in</Button>
        <Button onClick={viewport.fit}>Fit image</Button>
        {targets.length === 1 ? <Button onClick={() => viewport.focusRegion(targets[0])}>Zoom to target</Button> : null}
        <span className="tw:text-sm tw:text-muted-foreground">{Math.round(viewport.viewport.zoom * 100)}% of Fit</span>
      </div>
      <p id={helpId} className="tw:text-sm tw:text-muted-foreground">
        Recall the numbered target {targets.length === 1 ? "region" : "regions"}. Drag to pan, pinch to zoom, or use the view controls.
        With the image focused, arrow keys pan and + / − zoom. Image gestures never reveal or rate the answer.
      </p>
      <div className="tw:h-[min(65dvh,36rem)] tw:min-h-56 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted">
        <svg ref={viewport.svgRef} viewBox={`0 0 ${frame.width} ${frame.height}`}
          role="group" tabIndex={0} aria-label={label} aria-describedby={helpId}
          className="tw:block tw:h-full tw:w-full tw:touch-none tw:select-none tw:focus-visible:outline-2 tw:focus-visible:outline-primary"
          onPointerDown={viewport.pointerDown} onPointerMove={viewport.pointerMove} onPointerUp={viewport.pointerUp}
          onPointerCancel={viewport.pointerCancel} onLostPointerCapture={viewport.pointerCancel} onKeyDown={viewport.keyDown}>
          <image href={imageUrl} x={transform.left} y={transform.top} width={transform.width} height={transform.height}
            preserveAspectRatio="none" aria-hidden="true" />
          {targets.map((rect, index) => {
            const point = toScreenPoint(rect, transform);
            return <g key={index} aria-hidden="true" pointerEvents="none">
              <rect x={point.x} y={point.y} width={rect.width * transform.width} height={rect.height * transform.height}
                fill="none" className="tw:stroke-background" strokeWidth={6} />
              <rect x={point.x} y={point.y} width={rect.width * transform.width} height={rect.height * transform.height}
                fill="none" className="tw:stroke-foreground" strokeWidth={2} strokeDasharray="8 4" />
              <rect x={point.x} y={Math.max(0, point.y - 23)} width={28} height={22} className="tw:fill-background tw:stroke-foreground" />
              <text x={point.x + 8} y={Math.max(0, point.y - 23) + 16} className="tw:fill-foreground" fontSize={14}>{index + 1}</text>
            </g>;
          })}
        </svg>
      </div>
    </section>
  );
}
