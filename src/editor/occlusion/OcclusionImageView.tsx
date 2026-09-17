import { useId } from "react";

import type { NormalizedRect } from "../../ipc/questions";
import { Button } from "../../ui/primitives/Button";
import { toScreenPoint, type ImageViewport, type Size } from "./geometry";
import { useOcclusionWorkspace } from "./useOcclusionWorkspace";

interface OcclusionImageViewProps {
  imageUrl: string;
  imageKey: string;
  imageSize: Size;
  targets: readonly NormalizedRect[];
  label: string;
  initialViewport?: ImageViewport;
  onViewportChange?: (viewport: ImageViewport) => void;
}
const noSelection = () => undefined;

/** Navigation-only study view. Practice supplies an already-masked native raster;
 * this component never receives a source asset URL or chooses reveal state.
 */
export function OcclusionImageView({ imageUrl, imageKey, imageSize, targets, label, initialViewport, onViewportChange }: OcclusionImageViewProps) {
  const helpId = useId();
  const viewport = useOcclusionWorkspace({
    imageKey, imageSize, masks: [], selectedMaskId: null, navigationOnly: true, initialViewport, onViewportChange,
    onSelect: noSelection, onMutation: noSelection, onDelete: noSelection,
    onUndo: noSelection, onRedo: noSelection,
  });
  const { frame, transform } = viewport;
  return (
    <section className="tw:grid tw:min-w-0 tw:gap-2" aria-label={label}>
      <div className="tw:flex tw:items-center tw:gap-1" role="group" aria-label="Study image controls">
        <Button variant="ghost" size="icon" disabled={viewport.viewport.zoom <= 1}
          aria-label="Zoom out" title="Zoom out" onClick={() => viewport.zoom(1 / 1.5)}>
          <ZoomGlyph sign="minus" />
        </Button>
        <Button variant="ghost" size="icon" disabled={viewport.viewport.zoom >= 32}
          aria-label="Zoom in" title="Zoom in" onClick={() => viewport.zoom(1.5)}>
          <ZoomGlyph sign="plus" />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Drag to pan" title="Drag to pan"
          aria-pressed="true" className="tw:bg-accent" onClick={() => viewport.svgRef.current?.focus()}>
          <PanGlyph />
        </Button>
      </div>
      <p id={helpId} className="tw:sr-only">
        Recall the numbered target region. Drag the image to pan, pinch to zoom, use plus and minus to zoom,
        or use the arrow keys while the image is focused. Image gestures never reveal or rate the answer.
      </p>
      <div className="tw:h-[min(65dvh,36rem)] tw:min-h-56 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted">
        <svg ref={viewport.svgRef} viewBox={`0 0 ${frame.width} ${frame.height}`}
          data-occlusion-viewport="true" role="group" tabIndex={0} aria-label={label} aria-describedby={helpId}
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

function ZoomGlyph({ sign }: { sign: "plus" | "minus" }) {
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.8" />
    <path d="m15.5 15.5 5 5M7.5 10.5h6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    {sign === "plus" ? <path d="M10.5 7.5v6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /> : null}
  </svg>;
}

function PanGlyph() {
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path d="M8 11V6a1.5 1.5 0 0 1 3 0v4-6a1.5 1.5 0 0 1 3 0v6-4a1.5 1.5 0 0 1 3 0v5-2a1.5 1.5 0 0 1 3 0v4c0 5-3 8-8 8h-1c-3 0-5-1.3-6.5-3.5L2.5 14a1.7 1.7 0 0 1 2.8-1.9L8 15" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
  </svg>;
}
