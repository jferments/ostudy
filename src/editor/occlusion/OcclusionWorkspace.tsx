import { useId } from "react";

import type { NormalizedRect } from "../../ipc/questions";
import { Button } from "../../ui/primitives/Button";
import { SegmentedControl, type SegmentedOptions } from "../../ui/primitives/SegmentedControl";
import { moveRectangle, resizeRectangle, toScreenPoint, type Point } from "./geometry";
import type { OcclusionTool } from "./gestures";
import { RESIZE_HANDLES, useOcclusionWorkspace, type OcclusionWorkspaceOptions } from "./useOcclusionWorkspace";

const TOOLS = [
  { value: "select", label: "Select / adjust" }, { value: "rectangle", label: "Draw rectangle" },
  { value: "corners", label: "Tap two corners" }, { value: "pan", label: "Pan image" },
] as const satisfies SegmentedOptions<OcclusionTool>;

export interface OcclusionWorkspaceProps extends OcclusionWorkspaceOptions {
  imageUrl: string;
  canUndo: boolean;
  canRedo: boolean;
}

/** Only controlled rectangles and a managed raster URL enter this SVG. It is an
 * authoring workspace, not the Practice masking renderer. The parent owns drafts,
 * question bindings and geometry history; this component owns transient gestures.
 */
export function OcclusionWorkspace(props: OcclusionWorkspaceProps) {
  const workspace = useOcclusionWorkspace(props);
  const helpId = useId();
  const { frame, transform, interaction, selected } = workspace;
  const selectedRect = interaction.preview?.maskId === selected?.id
    ? interaction.preview?.rectangle : selected?.rect;
  const hitRadius = 22;

  function adjusted(rectangle: NormalizedRect): void {
    if (!selected || props.disabled) return;
    props.onMutation({ kind: "change", maskId: selected.id, before: selected.rect, after: rectangle });
  }
  function nudge(delta: Point): void {
    if (selected) adjusted(moveRectangle(selected.rect, delta));
  }

  return (
    <section className="tw:grid tw:min-w-0 tw:gap-3" aria-label="Image region editor">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2" role="group" aria-label="Image editing tools">
        <SegmentedControl label="Tool" value={workspace.tool} options={TOOLS} disabled={props.disabled} onChange={workspace.setTool} />
        <Button disabled={props.disabled || props.masks.length >= 128} onClick={workspace.addRegion}>Add rectangle</Button>
      </div>
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2" role="group" aria-label="Image view and history">
        <Button disabled={props.disabled || workspace.viewport.zoom <= 1} onClick={() => workspace.zoom(1 / 1.5)}>Zoom out</Button>
        <Button disabled={props.disabled || workspace.viewport.zoom >= 32} onClick={() => workspace.zoom(1.5)}>Zoom in</Button>
        <Button disabled={props.disabled} onClick={workspace.fit}>Fit image</Button>
        <Button disabled={props.disabled || !selected} onClick={() => { if (selected) workspace.focusRegion(selected.rect); }}>Zoom to region</Button>
        <Button disabled={props.disabled || !props.canUndo} onClick={() => { workspace.cancel(); props.onUndo(); }}>Undo region edit</Button>
        <Button disabled={props.disabled || !props.canRedo} onClick={() => { workspace.cancel(); props.onRedo(); }}>Redo region edit</Button>
        <span className="tw:text-small tw:text-muted-foreground">{Math.round(workspace.viewport.zoom * 100)}% of Fit</span>
      </div>
      <p id={helpId} className="tw:text-small tw:text-muted-foreground">
        {workspace.tool === "corners"
          ? interaction.firstCorner ? "Tap the opposite corner to finish the region." : "Tap one corner, then its opposite corner. No dragging is needed."
          : workspace.tool === "rectangle" ? "Drag across the label or region to hide. Two fingers pan and zoom without drawing."
          : "Select a region to adjust it. Use two fingers to pan and zoom, or use the view controls."}
        {" "}With the image focused, arrow keys move the selected region; Shift moves ten image pixels. Escape cancels a gesture.
      </p>
      <div className="tw:h-[min(65dvh,36rem)] tw:min-h-64 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted">
        <svg ref={workspace.svgRef} viewBox={`0 0 ${frame.width} ${frame.height}`}
          className="tw:block tw:h-full tw:w-full tw:touch-none tw:select-none tw:focus-visible:outline-2 tw:focus-visible:outline-primary"
          role="group" tabIndex={props.disabled ? -1 : 0} aria-label="Editable image regions" aria-describedby={helpId}
          onPointerDown={workspace.pointerDown} onPointerMove={workspace.pointerMove} onPointerUp={workspace.pointerUp}
          onPointerCancel={workspace.pointerCancel} onLostPointerCapture={workspace.pointerCancel} onKeyDown={workspace.keyDown}>
          <image href={props.imageUrl} x={transform.left} y={transform.top}
            width={transform.width} height={transform.height} preserveAspectRatio="none" aria-hidden="true" />
          {props.masks.map((mask, index) => {
            const rectangle = interaction.preview?.maskId === mask.id ? interaction.preview.rectangle : mask.rect;
            const position = toScreenPoint(rectangle, transform);
            return (
              <g key={mask.id} data-mask-id={mask.id} aria-hidden="true"
                className={mask.id === props.selectedMaskId ? "tw:text-primary" : "tw:text-foreground"}>
                <rect x={position.x} y={position.y} width={rectangle.width * transform.width}
                  height={rectangle.height * transform.height} fill="currentColor" fillOpacity={mask.id === props.selectedMaskId ? 0.25 : 0.45}
                  stroke="currentColor" strokeWidth={mask.id === props.selectedMaskId ? 3 : 2} />
                <text x={position.x + 4} y={position.y - 5} fontSize={14} fill="currentColor" pointerEvents="none">{index + 1}</text>
              </g>
            );
          })}
          {interaction.preview?.maskId === null ? (() => {
            const rect = interaction.preview.rectangle;
            const point = toScreenPoint(rect, transform);
            return <rect x={point.x} y={point.y} width={rect.width * transform.width} height={rect.height * transform.height}
              className="tw:text-primary" fill="currentColor" fillOpacity={0.2} stroke="currentColor" strokeWidth={2} strokeDasharray="6 4" pointerEvents="none" />;
          })() : null}
          {interaction.firstCorner ? (() => {
            const point = toScreenPoint(interaction.firstCorner, transform);
            return <circle cx={point.x} cy={point.y} r={6} className="tw:text-primary" fill="currentColor" pointerEvents="none" />;
          })() : null}
          {workspace.tool === "select" && selected && selectedRect ? RESIZE_HANDLES.map((handle) => {
            const point = toScreenPoint({
              x: selectedRect.x + (handle.endsWith("e") ? selectedRect.width : 0),
              y: selectedRect.y + (handle.startsWith("s") ? selectedRect.height : 0),
            }, transform);
            return <g key={handle} data-mask-id={selected.id} data-resize-handle={handle} aria-hidden="true" className="tw:text-primary">
              <rect x={point.x - hitRadius} y={point.y - hitRadius} width={2 * hitRadius} height={2 * hitRadius} fill="transparent" />
              <rect x={point.x - 5} y={point.y - 5} width={10} height={10} className="tw:fill-background tw:stroke-primary" strokeWidth={2} pointerEvents="none" />
            </g>;
          }) : null}
        </svg>
      </div>
      <div className="tw:flex tw:flex-wrap tw:gap-2" role="group" aria-label="Regions">
        {props.masks.map((mask, index) => (
          <Button key={mask.id} disabled={props.disabled} aria-pressed={mask.id === props.selectedMaskId}
            variant={mask.id === props.selectedMaskId ? "primary" : "secondary"}
            onClick={() => { workspace.cancel(); props.onSelect(mask.id); workspace.setTool("select"); }}>
            Region {index + 1}
          </Button>
        ))}
        {props.masks.length === 0 ? <p className="tw:text-small tw:text-muted-foreground">Add a rectangle, draw a region, or tap two corners.</p> : null}
      </div>
      {selected ? (
        <div className="tw:grid tw:gap-2" role="group" aria-label="Adjust selected region without dragging">
          <div className="tw:flex tw:flex-wrap tw:gap-2">
            <Button disabled={props.disabled} onClick={() => nudge({ x: -1 / props.imageSize.width, y: 0 })}>Move left</Button>
            <Button disabled={props.disabled} onClick={() => nudge({ x: 1 / props.imageSize.width, y: 0 })}>Move right</Button>
            <Button disabled={props.disabled} onClick={() => nudge({ x: 0, y: -1 / props.imageSize.height })}>Move up</Button>
            <Button disabled={props.disabled} onClick={() => nudge({ x: 0, y: 1 / props.imageSize.height })}>Move down</Button>
          </div>
          <div className="tw:flex tw:flex-wrap tw:gap-2">
            <Button disabled={props.disabled} onClick={() => adjusted(resizeRectangle(selected.rect, "se", { x: 1 / props.imageSize.width, y: 0 }, props.imageSize))}>Wider</Button>
            <Button disabled={props.disabled} onClick={() => adjusted(resizeRectangle(selected.rect, "se", { x: -1 / props.imageSize.width, y: 0 }, props.imageSize))}>Narrower</Button>
            <Button disabled={props.disabled} onClick={() => adjusted(resizeRectangle(selected.rect, "se", { x: 0, y: 1 / props.imageSize.height }, props.imageSize))}>Taller</Button>
            <Button disabled={props.disabled} onClick={() => adjusted(resizeRectangle(selected.rect, "se", { x: 0, y: -1 / props.imageSize.height }, props.imageSize))}>Shorter</Button>
            <Button variant="danger" disabled={props.disabled} onClick={() => { workspace.cancel(); props.onDelete(selected.id); }}>Remove region</Button>
          </div>
        </div>
      ) : null}
      <p role="status" className="tw:min-h-5 tw:text-small tw:text-muted-foreground">{workspace.status}</p>
    </section>
  );
}
