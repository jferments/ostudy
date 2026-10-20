import { useState } from "react";

import { Icon, type IconName } from "../../../ui/icons/Icon";
import { cn } from "../../../ui/internal/cn";
import { Sheet } from "../../../ui/overlays/Dialog";
import { OverflowMenu } from "../../../ui/overlays/OverflowMenu";
import { Toolbar, ToolbarAction, ToolbarGroup } from "../../../ui/primitives/Toolbar";
import {
  isShapeTool,
  SHAPE_TOOLS,
  STYLE_CHOICES,
  type AnnotatorTool,
  type EraseMode,
  type ShapeTool,
  type StyleFamily,
  type ToolStyle,
} from "./annotationModel";

interface ToolDescription { label: string; icon: IconName; shortcut: string; }

export const TOOLS: Readonly<Record<AnnotatorTool, ToolDescription>> = {
  select: { label: "Select", icon: "pointer", shortcut: "V" },
  lasso: { label: "Lasso", icon: "lasso", shortcut: "Q" },
  pen: { label: "Pen", icon: "pen", shortcut: "P" },
  highlighter: { label: "Highlighter", icon: "highlighter", shortcut: "H" },
  text: { label: "Text", icon: "text", shortcut: "T" },
  sticky: { label: "Sticky note", icon: "sticky-note", shortcut: "S" },
  line: { label: "Line", icon: "shape-line", shortcut: "L" },
  arrow: { label: "Arrow", icon: "shape-arrow", shortcut: "A" },
  rectangle: { label: "Box", icon: "shape-rectangle", shortcut: "R" },
  ellipse: { label: "Ellipse", icon: "shape-ellipse", shortcut: "O" },
  eraser: { label: "Eraser", icon: "eraser", shortcut: "E" },
};

const MAIN_TOOLS: readonly AnnotatorTool[] = ["select", "lasso", "pen", "highlighter"];
const NOTE_TOOLS: readonly AnnotatorTool[] = ["text", "sticky"];

interface AnnotatorToolbarProps {
  compact: boolean;
  disabled: boolean;
  tool: AnnotatorTool;
  /** The shape the compact Shapes menu shows when no shape tool is active. */
  lastShape: ShapeTool;
  /** The note tool the compact Notes menu shows when neither note tool is active. */
  lastNote: "text" | "sticky";
  onTool: (tool: AnnotatorTool) => void;
  /** The style the color and size choices change: the selection's, or the tool's. */
  style: { family: StyleFamily; style: ToolStyle } | null;
  onStyle: (patch: Partial<ToolStyle>) => void;
  selection: { isText: boolean; isSticky: boolean } | null;
  onDeleteSelection: () => void;
  onEditSelection: () => void;
  /** How many annotations the lasso took. */
  groupSize: number;
  onDeleteGroup: () => void;
  /** Opens a new card made from the selected annotation. */
  onMakeCard?: () => void;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  /** Pen-only drawing is offered once a pen has touched the page. */
  penSeen: boolean;
  penOnly: boolean;
  onTogglePenOnly: () => void;
  eraseMode: EraseMode;
  onEraseMode: (mode: EraseMode) => void;
  zoomPercent: number;
  fit: boolean;
  canZoomIn: boolean;
  canZoomOut: boolean;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFit: () => void;
}

/**
 * Tools, undo and zoom on the first row; the color and size of what is drawn
 * next (or of the selected annotation) on the second. A phone keeps each row
 * within one screen width: the note tools, shape tools and zoom fold into menus, undo moves
 * to the second row, and color and size open in a sheet.
 */
export function AnnotatorToolbar(props: AnnotatorToolbarProps) {
  const { compact, tool } = props;
  const shapeActive = isShapeTool(tool);
  const toolButton = (name: AnnotatorTool) => (
    <ToolbarAction key={name} label={TOOLS[name].label} pressed={tool === name}
      aria-keyshortcuts={TOOLS[name].shortcut} onClick={() => props.onTool(name)}>
      <Icon name={TOOLS[name].icon} />
    </ToolbarAction>
  );
  const shownShape = shapeActive ? tool : props.lastShape;
  const noteActive = NOTE_TOOLS.includes(tool);
  const shownNote = noteActive ? tool : props.lastNote;
  const history = [
    <ToolbarAction key="undo" label="Undo" disabled={!props.canUndo} aria-keyshortcuts="Control+Z Meta+Z" onClick={props.onUndo}>
      <Icon name="undo" />
    </ToolbarAction>,
    <ToolbarAction key="redo" label="Redo" disabled={!props.canRedo} aria-keyshortcuts="Control+Shift+Z Meta+Shift+Z Control+Y"
      onClick={props.onRedo}>
      <Icon name="redo" />
    </ToolbarAction>,
  ];
  const penOnly = props.penSeen ? (
    <ToolbarAction label="Draw with pen only" pressed={props.penOnly} onClick={props.onTogglePenOnly}>
      Pen only
    </ToolbarAction>
  ) : null;
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:px-page tw:py-1.5 tw:compact:px-1">
      <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2 tw:overflow-x-auto tw:compact:gap-1">
        <Toolbar label="Annotation tools" disabled={props.disabled} className="tw:flex-1 tw:flex-nowrap tw:gap-3 tw:compact:gap-1">
          <ToolbarGroup label="Tools" className="tw:flex-nowrap">
            {MAIN_TOOLS.map(toolButton)}
            {compact ? (
              <OverflowMenu
                label={noteActive ? `Notes: ${TOOLS[tool].label}` : "Notes"}
                triggerIcon={TOOLS[shownNote].icon}
                className={cn(noteActive && "tw:border-primary tw:bg-accent tw:text-accent-foreground")}
                disabled={props.disabled}
                items={NOTE_TOOLS.map((note) => ({
                  id: note, label: TOOLS[note].label, onSelect: () => props.onTool(note),
                }))}
              />
            ) : NOTE_TOOLS.map(toolButton)}
            {compact ? (
              <OverflowMenu
                label={shapeActive ? `Shapes: ${TOOLS[tool].label}` : "Shapes"}
                triggerIcon={TOOLS[shownShape].icon}
                className={cn(shapeActive && "tw:border-primary tw:bg-accent tw:text-accent-foreground")}
                disabled={props.disabled}
                items={SHAPE_TOOLS.map((shape) => ({
                  id: shape, label: TOOLS[shape].label, onSelect: () => props.onTool(shape),
                }))}
              />
            ) : SHAPE_TOOLS.map(toolButton)}
            {toolButton("eraser")}
          </ToolbarGroup>
          {compact ? null : (
            <>
              <ToolbarGroup label="History" className="tw:flex-nowrap">{history}</ToolbarGroup>
              {penOnly ? <ToolbarGroup label="Input">{penOnly}</ToolbarGroup> : null}
              <ToolbarGroup label="Zoom" className="tw:ml-auto tw:flex-nowrap">
                <ToolbarAction label="Zoom out" disabled={!props.canZoomOut} aria-keyshortcuts="Control+- Meta+-" onClick={props.onZoomOut}>
                  <Icon name="zoom-out" />
                </ToolbarAction>
                <ToolbarAction label={`Automatic zoom (now ${props.zoomPercent}%)`} pressed={props.fit}
                  aria-keyshortcuts="Control+0 Meta+0" onClick={props.onFit} className="tw:min-w-14 tw:tabular-nums">
                  {props.zoomPercent}%
                </ToolbarAction>
                <ToolbarAction label="Zoom in" disabled={!props.canZoomIn} aria-keyshortcuts="Control+= Meta+=" onClick={props.onZoomIn}>
                  <Icon name="zoom-in" />
                </ToolbarAction>
              </ToolbarGroup>
            </>
          )}
        </Toolbar>
        {compact ? (
          <OverflowMenu
            label={`Zoom (${props.zoomPercent}%)`}
            triggerIcon="zoom-in"
            disabled={props.disabled}
            items={[
              { id: "zoom-in", label: "Zoom in", onSelect: props.onZoomIn, disabled: !props.canZoomIn },
              { id: "zoom-out", label: "Zoom out", onSelect: props.onZoomOut, disabled: !props.canZoomOut },
              { id: "fit", label: "Automatic zoom", onSelect: props.onFit, disabled: props.fit },
            ]}
          />
        ) : null}
      </div>
      {compact ? (
        <div className="tw:flex tw:min-h-touch tw:min-w-0 tw:items-center tw:gap-1">
          <Toolbar label="History" disabled={props.disabled} className="tw:flex-nowrap tw:gap-1">{history}</Toolbar>
          <StyleRow {...props} />
          {penOnly ? <Toolbar label="Input" disabled={props.disabled} className="tw:ml-auto tw:flex-nowrap">{penOnly}</Toolbar> : null}
        </div>
      ) : <StyleRow {...props} />}
    </div>
  );
}

const DOT_SIZES = ["tw:size-1.5", "tw:size-2.5", "tw:size-3.5"] as const;
const SAMPLE_DOT_SIZES = ["tw:size-3", "tw:size-4", "tw:size-5"] as const;
const LETTER_SIZES = ["tw:text-caption", "tw:text-body", "tw:text-section"] as const;

const HINTS: Partial<Record<AnnotatorTool, string>> = {
  select: "Tap an annotation to select it.",
  lasso: "Draw a loop around annotations to select them together.",
  eraser: "Drag across annotations to erase them.",
};

const ERASE_HINTS: Record<EraseMode, string> = {
  whole: "Drag across annotations to erase them.",
  part: "Drag across pen and highlighter strokes to erase just that part.",
};

type StyleChoices = (typeof STYLE_CHOICES)[StyleFamily];
type ActiveStyle = NonNullable<AnnotatorToolbarProps["style"]>;

function swatchActions(style: ActiveStyle, choices: StyleChoices, onStyle: AnnotatorToolbarProps["onStyle"]) {
  return choices.swatches.map((swatch) => (
    <ToolbarAction key={swatch.color} label={swatch.label} pressed={style.style.color === swatch.color}
      onClick={() => onStyle({ color: swatch.color })}>
      <span aria-hidden="true" className="tw:size-5 tw:rounded-full tw:ring-1 tw:ring-foreground/30"
        style={{ backgroundColor: swatch.color }} />
    </ToolbarAction>
  ));
}

function sizeActions(style: ActiveStyle, choices: StyleChoices, onStyle: AnnotatorToolbarProps["onStyle"]) {
  return choices.sizes.map((choice, index) => (
    <ToolbarAction key={choice.size} label={choice.label} pressed={style.style.size === choice.size}
      onClick={() => onStyle({ size: choice.size })}>
      {isLetterFamily(style.family) ? (
        <span aria-hidden="true" className={cn("tw:font-semibold tw:leading-none", LETTER_SIZES[index])}>A</span>
      ) : (
        <span aria-hidden="true" className={cn("tw:rounded-full tw:bg-foreground", DOT_SIZES[index])} />
      )}
    </ToolbarAction>
  ));
}

const isLetterFamily = (family: StyleFamily) => family === "text" || family === "sticky";
const sizeGroupLabel = (style: ActiveStyle) => isLetterFamily(style.family) ? "Text size" : "Line width";

/** Always one row high, so the pages never jump when the tool changes. */
function StyleRow(props: AnnotatorToolbarProps) {
  const { compact, tool, style, onStyle, selection, onDeleteSelection, onEditSelection, onMakeCard, disabled } = props;
  const choices = style ? STYLE_CHOICES[style.family] : null;
  if (tool === "lasso" && props.groupSize > 0) {
    return (
      <div className="tw:flex tw:min-h-7 tw:min-w-0 tw:items-center tw:gap-2 tw:coarse:min-h-touch">
        <Toolbar label="Selected annotations" disabled={disabled} className="tw:flex-nowrap tw:gap-1">
          <ToolbarAction label={`Delete ${props.groupSize} selected`} aria-keyshortcuts="Delete Backspace"
            onClick={props.onDeleteGroup} className="tw:text-destructive">
            <Icon name="delete" />
          </ToolbarAction>
        </Toolbar>
        <span className="tw:truncate tw:text-label tw:text-muted-foreground">
          {props.groupSize} selected{compact ? "" : ". Drag them to move them together."}
        </span>
      </div>
    );
  }
  if (tool === "eraser" && !selection) {
    const { eraseMode, onEraseMode } = props;
    return (
      <div className="tw:flex tw:min-h-7 tw:min-w-0 tw:items-center tw:gap-2 tw:coarse:min-h-touch">
        <Toolbar label="Eraser" disabled={disabled} className="tw:flex-nowrap tw:gap-1">
          <ToolbarAction label="Erase whole annotations" pressed={eraseMode === "whole"} onClick={() => onEraseMode("whole")}>
            <Icon name="eraser" />
          </ToolbarAction>
          <ToolbarAction label="Erase part of a stroke" pressed={eraseMode === "part"} onClick={() => onEraseMode("part")}>
            <Icon name="scissors" />
          </ToolbarAction>
        </Toolbar>
        {compact ? null : <span className="tw:truncate tw:text-label tw:text-muted-foreground">{ERASE_HINTS[eraseMode]}</span>}
      </div>
    );
  }
  if (!choices && !selection) {
    return (
      <p className="tw:m-0 tw:flex tw:min-h-7 tw:min-w-0 tw:items-center tw:px-1 tw:text-label tw:text-muted-foreground tw:coarse:min-h-touch">
        <span className="tw:truncate">{HINTS[tool] ?? ""}</span>
      </p>
    );
  }
  const selectionActions = selection ? (
    <ToolbarGroup label="Selection" className="tw:flex-nowrap">
      {selection.isText ? (
        <ToolbarAction label={selection.isSticky ? "Open note" : "Edit text"} onClick={onEditSelection}>
          <Icon name="edit" />
        </ToolbarAction>
      ) : null}
      {onMakeCard ? (
        <ToolbarAction label="Make a card" onClick={onMakeCard}>
          <Icon name="cards" />
        </ToolbarAction>
      ) : null}
      <ToolbarAction label="Delete" aria-keyshortcuts="Delete Backspace" onClick={onDeleteSelection}
        className="tw:text-destructive">
        <Icon name="delete" />
      </ToolbarAction>
    </ToolbarGroup>
  ) : null;
  const label = selection ? "Selected annotation" : "Color and size";
  if (compact) {
    return (
      <Toolbar label={label} disabled={disabled} className="tw:flex-nowrap tw:gap-1">
        {style && choices ? <StyleSheetButton style={style} choices={choices} onStyle={onStyle} /> : null}
        {selectionActions}
      </Toolbar>
    );
  }
  return (
    <div className="tw:flex tw:min-h-7 tw:min-w-0 tw:items-center tw:gap-2 tw:overflow-x-auto tw:coarse:min-h-touch">
      <Toolbar label={label} disabled={disabled} className="tw:flex-nowrap tw:gap-3">
        {style && choices ? (
          <>
            <ToolbarGroup label="Color" className="tw:flex-nowrap">{swatchActions(style, choices, onStyle)}</ToolbarGroup>
            <ToolbarGroup label={sizeGroupLabel(style)} className="tw:flex-nowrap">{sizeActions(style, choices, onStyle)}</ToolbarGroup>
          </>
        ) : null}
        {selectionActions}
      </Toolbar>
    </div>
  );
}

/** A phone's color and size: one button showing both, which opens the choices in a sheet. */
function StyleSheetButton({ style, choices, onStyle }: {
  style: ActiveStyle;
  choices: StyleChoices;
  onStyle: AnnotatorToolbarProps["onStyle"];
}) {
  const [open, setOpen] = useState(false);
  const sizeIndex = choices.sizes.findIndex((choice) => choice.size === style.style.size);
  const summary = [
    choices.swatches.find((swatch) => swatch.color === style.style.color)?.label,
    choices.sizes[sizeIndex]?.label,
  ].filter(Boolean).join(", ");
  return (
    <>
      <ToolbarAction label={summary ? `Color and size: ${summary}` : "Color and size"} aria-haspopup="dialog"
        aria-expanded={open} onClick={() => setOpen(true)}>
        {isLetterFamily(style.family) ? (
          <span aria-hidden="true" className={cn("tw:font-semibold tw:leading-none", LETTER_SIZES[Math.max(0, sizeIndex)])}
            style={{ color: style.style.color }}>A</span>
        ) : (
          <span aria-hidden="true" className={cn("tw:rounded-full tw:ring-1 tw:ring-foreground/30", SAMPLE_DOT_SIZES[Math.max(0, sizeIndex)])}
            style={{ backgroundColor: style.style.color }} />
        )}
      </ToolbarAction>
      {open ? (
        <Sheet title="Color and size" width="small" onClose={() => setOpen(false)}>
          <div className="tw:grid tw:gap-4">
            <div className="tw:grid tw:gap-2">
              <p className="tw:m-0 tw:text-label tw:font-medium tw:text-muted-foreground">Color</p>
              <Toolbar label="Color" className="tw:gap-1">{swatchActions(style, choices, onStyle)}</Toolbar>
            </div>
            <div className="tw:grid tw:gap-2">
              <p className="tw:m-0 tw:text-label tw:font-medium tw:text-muted-foreground">{sizeGroupLabel(style)}</p>
              <Toolbar label={sizeGroupLabel(style)} className="tw:gap-1">{sizeActions(style, choices, onStyle)}</Toolbar>
            </div>
          </div>
        </Sheet>
      ) : null}
    </>
  );
}
