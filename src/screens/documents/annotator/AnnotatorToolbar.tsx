import { useState, type MouseEvent, type ReactNode } from "react";

import { Icon, type IconName } from "../../../ui/icons/Icon";
import { cn } from "../../../ui/internal/cn";
import { Sheet } from "../../../ui/overlays/Dialog";
import { OverflowMenu, type OverflowMenuItem } from "../../../ui/overlays/OverflowMenu";
import { Toolbar, ToolbarAction, ToolbarGroup } from "../../../ui/primitives/Toolbar";
import {
  isShapeTool,
  SHAPE_TOOLS,
  STYLE_CHOICES,
  type ActiveStyle,
  type AnnotatorTool,
  type EraseMode,
  type ShapeTool,
  type StyleFamily,
  type ToolStyle,
} from "./annotationModel";
import { ColorChoices, ShapeLookChoices, SizeField } from "./StyleChoices";
import type { TextFormat } from "./textFormat";

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
  curve: { label: "Curve", icon: "shape-curve", shortcut: "C" },
  rectangle: { label: "Box", icon: "shape-rectangle", shortcut: "R" },
  ellipse: { label: "Ellipse", icon: "shape-ellipse", shortcut: "O" },
  eraser: { label: "Eraser", icon: "eraser", shortcut: "E" },
  clip: { label: "Clip to notes", icon: "crop", shortcut: "X" },
  hand: { label: "Hand", icon: "pan", shortcut: "Space" },
  mindmap: { label: "Mind map", icon: "mind-map", shortcut: "M" },
};

const MAIN_TOOLS: readonly AnnotatorTool[] = ["select", "lasso", "pen", "highlighter"];
const PICK_TOOLS: readonly AnnotatorTool[] = ["select", "lasso"];
/** Marking and unmarking sit together, as in note-taking and PDF apps. */
const MARK_TOOLS: readonly AnnotatorTool[] = ["pen", "highlighter", "eraser"];

/** A thin rule between groups of controls, so each group reads as one. */
function Divider() {
  return <span aria-hidden="true" className="tw:mx-0.5 tw:h-5 tw:w-px tw:shrink-0 tw:bg-border" />;
}
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
  style: ActiveStyle | null;
  onStyle: (patch: Partial<ToolStyle>) => void;
  /** Lists and formulas, while a text note is typed or selected. */
  textFormat?: TextFormat | null;
  selection: { isText: boolean; isSticky: boolean; isCollapsed: boolean; isLink?: boolean; linkRemoved?: boolean } | null;
  /** Follows the selected link. */
  onOpenLink?: () => void;
  /** Offers Add link among the tools that put something on the page. */
  onAddLink?: () => void;
  /** Where the reader is and whether marks are saved, at the toolbar's end. */
  status?: ReactNode;
  /** Folds the selected sticky note to its icon, or opens it back out. */
  onToggleCollapsed: () => void;
  onDeleteSelection: () => void;
  onEditSelection: () => void;
  /** Offers the Clip tool, which copies part of a PDF page into a notebook. */
  canClip?: boolean;
  /** Offers the Hand tool, which drags the view around a board. */
  canPan?: boolean;
  /** Offers the Mind map tool, on a notebook's pages and boards. */
  canMap?: boolean;
  /** The Mind map tool's dropdown: what can be done with maps and the selected topic. */
  mapMenu?: ReadonlyArray<OverflowMenuItem>;
  /** The fit button fits everything on a board in the window rather than the page width. */
  fitAll?: boolean;
  /** How many annotations the lasso took. */
  groupSize: number;
  onDeleteGroup: () => void;
  /** Whether the selected annotations are one saved group, which moves and selects together. */
  isOneGroup: boolean;
  /** Joins the selected annotations into a group, or ungroups them. */
  onGroup: (grouped: boolean) => void;
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
 * One row of tools in groups (pick, mark, add, history, view), then one row
 * that shows only what the active tool or selection can change, as graphics
 * and PDF editors do. The shapes share one button that keeps the last shape
 * and lists the others. A phone keeps each row within one screen width: the
 * note tools, shape tools and zoom fold into menus, undo moves to the second
 * row, and color and size open in a sheet.
 */
export function AnnotatorToolbar(props: AnnotatorToolbarProps) {
  const { compact, tool } = props;
  const shapeActive = isShapeTool(tool);
  const toolButton = (name: AnnotatorTool) => (
    <ToolbarAction key={name} label={TOOLS[name].label} title={`${TOOLS[name].label} (${TOOLS[name].shortcut})`}
      pressed={tool === name} aria-keyshortcuts={TOOLS[name].shortcut} onClick={() => props.onTool(name)}>
      <Icon name={TOOLS[name].icon} />
    </ToolbarAction>
  );
  const shownShape = shapeActive ? tool : props.lastShape;
  const mainTools = props.canPan ? [...MAIN_TOOLS.slice(0, 1), "hand" as const, ...MAIN_TOOLS.slice(1)] : MAIN_TOOLS;
  const pickTools = props.canPan ? [...PICK_TOOLS, "hand" as const] : PICK_TOOLS;
  const fitLabel = props.fitAll ? "Fit all" : "Automatic zoom";
  const noteActive = NOTE_TOOLS.includes(tool);
  const shownNote = noteActive ? tool : props.lastNote;
  const history = [
    <ToolbarAction key="undo" label="Undo" title="Undo (Ctrl+Z)" disabled={!props.canUndo} aria-keyshortcuts="Control+Z Meta+Z" onClick={props.onUndo}>
      <Icon name="undo" />
    </ToolbarAction>,
    <ToolbarAction key="redo" label="Redo" title="Redo (Ctrl+Shift+Z)" disabled={!props.canRedo} aria-keyshortcuts="Control+Shift+Z Meta+Shift+Z Control+Y"
      onClick={props.onRedo}>
      <Icon name="redo" />
    </ToolbarAction>,
  ];
  const penOnly = props.penSeen ? (
    <ToolbarAction label="Draw with pen only" pressed={props.penOnly} onClick={props.onTogglePenOnly}>
      Pen only
    </ToolbarAction>
  ) : null;
  const addLink = props.onAddLink ? (
    <ToolbarAction key="link" label="Add link" title="Add link (K)" aria-keyshortcuts="K" onClick={props.onAddLink}>
      <Icon name="link" />
    </ToolbarAction>
  ) : null;
  // One Mind map button; the map's actions open from the arrow beside it.
  const mapTool = props.canMap ? (
    <span className="tw:flex tw:items-center">
      {toolButton("mindmap")}
      {props.mapMenu ? (
        <OverflowMenu
          label="Mind map actions"
          triggerIcon="chevron-down"
          className="tw:w-5 tw:min-w-5 tw:px-0"
          disabled={props.disabled}
          items={props.mapMenu}
        />
      ) : null}
    </span>
  ) : null;
  const status = props.status ? (
    <div className="tw:flex tw:shrink-0 tw:items-center tw:gap-2 tw:text-label tw:whitespace-nowrap tw:text-muted-foreground">
      {props.status}
    </div>
  ) : null;
  if (compact) {
    return (
      <div className="tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:px-1 tw:py-1.5">
        <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-1 tw:overflow-x-auto">
          <Toolbar label="Annotation tools" disabled={props.disabled} className="tw:flex-1 tw:flex-nowrap tw:gap-1">
            <ToolbarGroup label="Tools" className="tw:flex-nowrap">
              {mainTools.map(toolButton)}
              <OverflowMenu
                label={noteActive ? `Notes: ${TOOLS[tool].label}` : "Notes"}
                triggerIcon={TOOLS[shownNote].icon}
                className={cn(noteActive && "tw:border-primary tw:bg-accent tw:text-accent-foreground")}
                disabled={props.disabled}
                items={NOTE_TOOLS.map((note) => ({
                  id: note, label: TOOLS[note].label, onSelect: () => props.onTool(note),
                }))}
              />
              <OverflowMenu
                label={shapeActive ? `Shapes: ${TOOLS[tool].label}` : "Shapes"}
                triggerIcon={TOOLS[shownShape].icon}
                className={cn(shapeActive && "tw:border-primary tw:bg-accent tw:text-accent-foreground")}
                disabled={props.disabled}
                items={SHAPE_TOOLS.map((shape) => ({
                  id: shape, label: TOOLS[shape].label, onSelect: () => props.onTool(shape),
                }))}
              />
              {toolButton("eraser")}
              {mapTool}
              {addLink}
              {props.canClip ? toolButton("clip") : null}
            </ToolbarGroup>
          </Toolbar>
          <OverflowMenu
            label={`Zoom (${props.zoomPercent}%)`}
            triggerIcon="zoom-in"
            disabled={props.disabled}
            items={[
              { id: "zoom-in", label: "Zoom in", onSelect: props.onZoomIn, disabled: !props.canZoomIn },
              { id: "zoom-out", label: "Zoom out", onSelect: props.onZoomOut, disabled: !props.canZoomOut },
              { id: "fit", label: fitLabel, onSelect: props.onFit, disabled: props.fit && !props.fitAll },
            ]}
          />
        </div>
        <div className="tw:flex tw:min-h-touch tw:min-w-0 tw:items-center tw:gap-1">
          <Toolbar label="History" disabled={props.disabled} className="tw:flex-nowrap tw:gap-1">{history}</Toolbar>
          <StyleRow {...props} />
          {penOnly ? <Toolbar label="Input" disabled={props.disabled} className="tw:ms-auto tw:flex-nowrap">{penOnly}</Toolbar> : null}
          {status ? <div className={cn(!penOnly && "tw:ms-auto")}>{status}</div> : null}
        </div>
      </div>
    );
  }
  return (
    <div className="tw:grid tw:min-w-0 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background">
      <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-3 tw:px-3 tw:py-1">
        <Toolbar label="Annotation tools" disabled={props.disabled} className="tw:flex-1 tw:flex-wrap tw:gap-x-2 tw:gap-y-1">
          <ToolbarGroup label="Pick" className="tw:flex-nowrap">{pickTools.map(toolButton)}</ToolbarGroup>
          <Divider />
          <ToolbarGroup label="Mark" className="tw:flex-nowrap">{MARK_TOOLS.map(toolButton)}</ToolbarGroup>
          <Divider />
          <ToolbarGroup label="Add" className="tw:flex-nowrap">
            {NOTE_TOOLS.map(toolButton)}
            <span className="tw:flex tw:items-center">
              {toolButton(shownShape)}
              <OverflowMenu
                label="More shapes"
                triggerIcon="chevron-down"
                className="tw:w-5 tw:min-w-5 tw:px-0"
                disabled={props.disabled}
                items={SHAPE_TOOLS.map((shape) => ({
                  id: shape,
                  label: TOOLS[shape].label,
                  checked: shape === shownShape,
                  onSelect: () => props.onTool(shape),
                }))}
              />
            </span>
            {mapTool}
            {addLink}
            {props.canClip ? toolButton("clip") : null}
          </ToolbarGroup>
          <Divider />
          <ToolbarGroup label="History" className="tw:flex-nowrap">{history}</ToolbarGroup>
          <div className="tw:ms-auto tw:flex tw:items-center tw:gap-2">
            {penOnly ? <ToolbarGroup label="Input">{penOnly}</ToolbarGroup> : null}
            <ToolbarGroup label="Zoom" className="tw:flex-nowrap">
              <ToolbarAction label="Zoom out" title="Zoom out (Ctrl+-)" disabled={!props.canZoomOut} aria-keyshortcuts="Control+- Meta+-" onClick={props.onZoomOut}>
                <Icon name="zoom-out" />
              </ToolbarAction>
              <ToolbarAction label={`${fitLabel} (now ${props.zoomPercent}%)`} title={props.fitAll ? "Fit all (Ctrl+0)" : "Fit to width (Ctrl+0)"}
                pressed={props.fit && !props.fitAll}
                aria-keyshortcuts="Control+0 Meta+0" onClick={props.onFit} className="tw:min-w-14 tw:tabular-nums">
                {props.zoomPercent}%
              </ToolbarAction>
              <ToolbarAction label="Zoom in" title="Zoom in (Ctrl+=)" disabled={!props.canZoomIn} aria-keyshortcuts="Control+= Meta+=" onClick={props.onZoomIn}>
                <Icon name="zoom-in" />
              </ToolbarAction>
            </ToolbarGroup>
          </div>
        </Toolbar>
        {status ? <><Divider />{status}</> : null}
      </div>
      {/* The active tool's or selection's settings, on a strip of their own. */}
      <div className="tw:min-w-0 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-muted/40 tw:px-3 tw:py-1">
        <StyleRow {...props} />
      </div>
    </div>
  );
}

const DOT_SIZES = ["tw:size-1.5", "tw:size-2.5", "tw:size-3.5"] as const;
const SAMPLE_DOT_SIZES = ["tw:size-3", "tw:size-4", "tw:size-5"] as const;
const LETTER_SIZES = ["tw:text-caption", "tw:text-body", "tw:text-section"] as const;

type StyleChoices = (typeof STYLE_CHOICES)[StyleFamily];

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
  if ((tool === "lasso" || tool === "select" || tool === "mindmap") && props.groupSize > 0 && !selection) {
    return (
      <div className="tw:flex tw:min-h-7 tw:min-w-0 tw:items-center tw:gap-2 tw:coarse:min-h-touch">
        <Toolbar label="Selected annotations" disabled={disabled} className="tw:flex-nowrap tw:gap-1">
          {props.isOneGroup ? (
            <ToolbarAction label="Ungroup" aria-keyshortcuts="Control+Shift+G Meta+Shift+G" onClick={() => props.onGroup(false)}>
              Ungroup
            </ToolbarAction>
          ) : props.groupSize > 1 ? (
            <ToolbarAction label="Group" aria-keyshortcuts="Control+G Meta+G" onClick={() => props.onGroup(true)}>
              Group
            </ToolbarAction>
          ) : null}
          <ToolbarAction label={`Delete ${props.groupSize} selected`} aria-keyshortcuts="Delete Backspace"
            onClick={props.onDeleteGroup} className="tw:text-destructive">
            <Icon name="delete" />
          </ToolbarAction>
        </Toolbar>
        <span className="tw:truncate tw:text-label tw:text-muted-foreground">
          {props.isOneGroup ? "Group of " : ""}{props.groupSize}{props.isOneGroup ? "" : " selected"}
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
      </div>
    );
  }
  // Nothing to set: the strip keeps its height, so the pages never jump.
  if (!choices && !selection) return <div aria-hidden="true" className="tw:min-h-7 tw:coarse:min-h-touch" />;
  const selectionActions = selection ? (
    <ToolbarGroup label="Selection" className="tw:flex-nowrap">
      {selection.isLink && props.onOpenLink ? (
        <ToolbarAction label={selection.linkRemoved ? "Link target removed" : "Open link"} onClick={props.onOpenLink}>
          <Icon name="arrow-right" />
        </ToolbarAction>
      ) : null}
      {selection.isText ? (
        <ToolbarAction label={selection.isLink ? "Edit link" : selection.isSticky ? "Open note" : "Edit text"} onClick={onEditSelection}>
          <Icon name="edit" />
        </ToolbarAction>
      ) : null}
      {selection.isSticky ? (
        <ToolbarAction label={selection.isCollapsed ? "Expand note" : "Collapse to an icon"} onClick={props.onToggleCollapsed}>
          <Icon name={selection.isCollapsed ? "maximize" : "minimize"} />
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
  const textActions = props.textFormat ? <TextFormatActions format={props.textFormat} /> : null;
  if (compact) {
    return (
      <Toolbar label={label} disabled={disabled} className="tw:flex-nowrap tw:gap-1">
        {style && choices ? <StyleSheetButton style={style} choices={choices} onStyle={onStyle} /> : null}
        {textActions}
        {selectionActions}
      </Toolbar>
    );
  }
  return (
    // A narrow pane, as beside a document, wraps the groups onto a second line.
    <div className="tw:flex tw:min-h-7 tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-x-3 tw:gap-y-1 tw:coarse:min-h-touch">
      {style && choices ? (
        <fieldset disabled={disabled} className="tw:m-0 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-x-3 tw:gap-y-1 tw:border-0 tw:p-0">
          <legend className="tw:sr-only">{selection ? "Style of the selected annotation" : label}</legend>
          <div className="tw:flex tw:shrink-0 tw:flex-nowrap tw:items-center"><ColorChoices style={style} onStyle={onStyle} /></div>
          <Divider />
          <div className="tw:flex tw:flex-nowrap tw:items-center tw:gap-1">
            <Toolbar label={sizeGroupLabel(style)} className="tw:flex-nowrap tw:gap-1">{sizeActions(style, choices, onStyle)}</Toolbar>
            <SizeField style={style} onStyle={onStyle} />
          </div>
          <ShapeLookChoices style={style} onStyle={onStyle} layout="row" divider={<Divider />} />
        </fieldset>
      ) : null}
      {textActions ? (
        <>
          <Divider />
          <Toolbar label="Text format" disabled={disabled} className="tw:flex-nowrap">{textActions}</Toolbar>
        </>
      ) : null}
      {selectionActions ? (
        // What can be done with the selection sits apart, at the row's end.
        <Toolbar label={label} disabled={disabled} className="tw:ms-auto tw:flex-nowrap tw:gap-3">
          {selectionActions}
        </Toolbar>
      ) : null}
    </div>
  );
}

/**
 * Bullets, numbers, checkboxes and formulas for a text note. Pressing one
 * keeps the note being typed open, so it acts on the lines being typed.
 */
function TextFormatActions({ format }: { format: TextFormat }) {
  const keepTyping = (event: MouseEvent) => event.preventDefault();
  return (
    <ToolbarGroup label="Text" className="tw:flex-nowrap">
      <ToolbarAction label="Bulleted list" onMouseDown={keepTyping} onClick={() => format.onList("bullet")}>
        <Icon name="list" />
      </ToolbarAction>
      <ToolbarAction label="Numbered list" onMouseDown={keepTyping} onClick={() => format.onList("number")}>
        <Icon name="list-ordered" />
      </ToolbarAction>
      <ToolbarAction label="Checklist" onMouseDown={keepTyping} onClick={() => format.onList("check")}>
        <Icon name="list-checks" />
      </ToolbarAction>
      <ToolbarAction label="Insert formula" onMouseDown={keepTyping} onClick={format.onFormula}>
        <Icon name="formula" />
      </ToolbarAction>
    </ToolbarGroup>
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
              <ColorChoices style={style} onStyle={onStyle} />
            </div>
            <div className="tw:grid tw:gap-2">
              <p className="tw:m-0 tw:text-label tw:font-medium tw:text-muted-foreground">{sizeGroupLabel(style)}</p>
              <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
                <Toolbar label={sizeGroupLabel(style)} className="tw:gap-1">{sizeActions(style, choices, onStyle)}</Toolbar>
                <SizeField style={style} onStyle={onStyle} />
              </div>
            </div>
            <ShapeLookChoices style={style} onStyle={onStyle} layout="sheet" />
          </div>
        </Sheet>
      ) : null}
    </>
  );
}
