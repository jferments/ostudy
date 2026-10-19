import { useId, useState, type MouseEvent } from "react";
import { Toolbar, ToolbarGroup, ToolbarAction } from "../../ui/primitives/Toolbar";
import { ColorField } from "../../ui/primitives/ColorField";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Button } from "../../ui/primitives/Button";
import { Icon } from "../../ui/icons/Icon";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import {
  editorToolbarGroups, type EditorAdapter, type EditorBlockStyle,
  type EditorCommandRequest, type EditorTextAlignment,
} from "../adapter";

const FONT_SIZES = [8, 10, 12, 14, 16, 18, 20, 24, 30, 36, 48, 60, 72, 84, 96];
const SYMBOLS = ["±", "→", "←", "↔", "↑", "↓", "≤", "≥", "≠", "≈", "∞", "°", "µ", "α", "β", "γ", "δ", "Δ", "λ", "π", "Σ", "√", "×", "÷", "•", "✓", "✕"];

interface EditorToolbarProps {
  adapter: EditorAdapter;
  onAddImage: () => void;
  onAddMedia: () => void;
  onOpenLink: () => void;
  onOpenFormula: () => void;
  onOpenTable: () => void;
  onOpenChecklist: () => void;
  isImportingImage: boolean;
  isImportingMedia: boolean;
  imageDraftReady: boolean;
  mediaDraftReady: boolean;
  disabled: boolean;
}

export function EditorToolbar({ adapter, onAddImage, onAddMedia, onOpenFormula, onOpenLink, onOpenTable, onOpenChecklist, isImportingImage, isImportingMedia, imageDraftReady, mediaDraftReady, disabled }: EditorToolbarProps) {
  const [advanced, setAdvanced] = useState(false);
  const advancedId = useId();
  const selection = adapter.selection();
  const textStyle = adapter.attributes("textStyle");
  const historyGroup = editorToolbarGroups(adapter.capabilities).find((group) => group.id === "history");
  function preserveSelection(event: MouseEvent<HTMLButtonElement>) { event.preventDefault(); }
  function run(request: EditorCommandRequest): void { if (!disabled) adapter.execute(request); }
  const advancedItems = (historyGroup?.items ?? []).map((item) => ({
    id: item.id, label: item.label,
    disabled: disabled || !adapter.canExecute({ command: item.command } as EditorCommandRequest),
    onSelect: () => run({ command: item.command } as EditorCommandRequest),
  }));
  return <fieldset disabled={disabled} className="tw:m-0 tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-muted tw:p-2 tw:disabled:opacity-60">
    <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
      <Toolbar label="Insert media and links" disabled={disabled}>
        <ToolbarGroup label="Quick insert">
          <ToolbarAction label="Insert image" onMouseDown={preserveSelection} onClick={onAddImage} disabled={isImportingImage || !imageDraftReady}>
            <Icon name="image" />
          </ToolbarAction>
          <ToolbarAction label="Add or edit link" pressed={adapter.isActive("link")} onMouseDown={preserveSelection} onClick={onOpenLink}>
            <Icon name="link" />
          </ToolbarAction>
          <ToolbarAction label="Insert or edit formula" onMouseDown={preserveSelection} onClick={onOpenFormula}>
            <Icon name="formula" />
          </ToolbarAction>
          <ToolbarAction label="Insert audio or video" onMouseDown={preserveSelection} onClick={onAddMedia} disabled={isImportingMedia || !mediaDraftReady}>
            <Icon name="media" />
          </ToolbarAction>
        </ToolbarGroup>
      </Toolbar>
      <Button variant="text" size="small" disabled={disabled} aria-expanded={advanced} aria-controls={advancedId}
        onMouseDown={preserveSelection} onClick={() => setAdvanced((value) => !value)}>
        Advanced editor <Icon name={advanced ? "chevron-down" : "chevron-right"} size={16} />
      </Button>
    </div>
    {advanced ? <div id={advancedId} className="tw:grid tw:min-w-0 tw:gap-2 tw:pt-1">
    {/* Native pickers are outside roving command navigation: their arrows select values. */}
    <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">
      <div data-compact-picker="true" className="tw:w-36 tw:shrink-0"><NativeSelect aria-label="Paragraph style" disabled={disabled} value={selection.blockStyle} onChange={(event) => run({ command: "set_block", value: event.currentTarget.value as EditorBlockStyle })}>
        <option value="paragraph">Normal</option><option value="h1">Heading 1</option><option value="h2">Heading 2</option><option value="h3">Heading 3</option><option value="h4">Heading 4</option>
      </NativeSelect></div>
      <div data-compact-picker="true" className="tw:w-40 tw:shrink-0"><NativeSelect aria-label="Font family" disabled={disabled} value={typeof textStyle.fontFamily === "string" ? textStyle.fontFamily : ""} onChange={(event) => run({ command: "set_font_family", value: event.currentTarget.value || null })}>
        <option value="">Default font</option><option value="Arial, sans-serif">Arial</option><option value="Georgia, serif">Georgia</option><option value="'Times New Roman', serif">Times New Roman</option><option value="'Courier New', monospace">Courier New</option><option value="monospace">Monospace</option>
      </NativeSelect></div>
      <div data-compact-picker="true" className="tw:w-20 tw:shrink-0"><NativeSelect aria-label="Text size" disabled={disabled} value={typeof textStyle.fontSize === "string" ? textStyle.fontSize : ""} onChange={(event) => run({ command: "set_font_size", value: event.currentTarget.value || null })}>
        <option value="">Size</option>{FONT_SIZES.map((size) => <option key={size} value={`${size}px`}>{size}</option>)}
      </NativeSelect></div>
      <OverflowMenu disabled={disabled} label="Lists" triggerPresentation="text" items={[
        { id: "bullet", label: "Bullet list", onSelect: () => run({ command: "toggle_bullet_list" }) },
        { id: "numbered", label: "Numbered list", onSelect: () => run({ command: "toggle_numbered_list" }) },
        { id: "checkbox", label: "Checkbox list", onSelect: () => run({ command: "toggle_task_list" }) },
      ]} />
    </div>
    <Toolbar label="Text formatting" disabled={disabled}>
      <ToolbarGroup label="Text marks">
        <ToolbarAction label="Bold" pressed={adapter.isActive("bold")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_bold" })}><strong>B</strong></ToolbarAction>
        <ToolbarAction label="Italic" pressed={adapter.isActive("italic")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_italic" })}><em>I</em></ToolbarAction>
        <ToolbarAction label="Underline" pressed={adapter.isActive("underline")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_underline" })}><u>U</u></ToolbarAction>
        <ToolbarAction label="Subscript" pressed={adapter.isActive("subscript")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_subscript" })}>X<sub>2</sub></ToolbarAction>
        <ToolbarAction label="Superscript" pressed={adapter.isActive("superscript")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_superscript" })}>X<sup>2</sup></ToolbarAction>
      </ToolbarGroup>
      <ToolbarGroup label="Lists and indentation">
        {selection.inTaskList ? <ToolbarAction label={selection.taskChecked ? "Mark task incomplete" : "Mark task complete"} pressed={selection.taskChecked === true} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_task_checked" })}>✓</ToolbarAction> : null}
        <ToolbarAction label="Decrease indentation" onMouseDown={preserveSelection} onClick={() => run({ command: "indent", value: -1 })}>⇤</ToolbarAction>
        <ToolbarAction label="Increase indentation" onMouseDown={preserveSelection} onClick={() => run({ command: "indent", value: 1 })}>⇥</ToolbarAction>
      </ToolbarGroup>
      <ToolbarGroup label="Alignment">
        {(["left", "center", "right"] as const).map((alignment) => <ToolbarAction key={alignment} label={`Align ${alignment}`} pressed={selection.textAlignment === alignment} onMouseDown={preserveSelection} onClick={() => run({ command: "set_text_alignment", value: alignment as EditorTextAlignment })}>{alignment === "left" ? "≡←" : alignment === "center" ? "≡" : "→≡"}</ToolbarAction>)}
      </ToolbarGroup>
      {selection.inTable ? <ToolbarGroup label="Table editing">
        <ToolbarAction label="Add table row" onMouseDown={preserveSelection} onClick={() => run({ command: "add_table_row" })}>+ Row</ToolbarAction>
        <ToolbarAction label="Delete table row" onMouseDown={preserveSelection} onClick={() => run({ command: "delete_table_row" })}>− Row</ToolbarAction>
        <ToolbarAction label="Add table column" onMouseDown={preserveSelection} onClick={() => run({ command: "add_table_column" })}>+ Column</ToolbarAction>
        <ToolbarAction label="Delete table column" onMouseDown={preserveSelection} onClick={() => run({ command: "delete_table_column" })}>− Column</ToolbarAction>
        <ToolbarAction label="Toggle table header row" onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_table_header_row" })}>Header</ToolbarAction>
        <ToolbarAction label="Delete table" onMouseDown={preserveSelection} onClick={() => run({ command: "delete_table" })}>Delete table</ToolbarAction>
      </ToolbarGroup> : null}
    </Toolbar>
    <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:gap-3">
      <label className="tw:grid tw:gap-1 tw:text-caption tw:text-muted-foreground">Text color<ColorField disabled={disabled} aria-label="Text color" value={normalizeColor(textStyle.color, "#d9e2f2")} onChange={(event) => run({ command: "set_text_color", value: event.currentTarget.value })} /></label>
      <label className="tw:grid tw:gap-1 tw:text-caption tw:text-muted-foreground">Highlight<ColorField disabled={disabled} aria-label="Highlight color" value="#ffd166" onChange={(event) => run({ command: "set_highlight", value: event.currentTarget.value })} /></label>
      <div className="tw:w-28"><NativeSelect disabled={disabled} aria-label="Insert symbol" defaultValue="" onChange={(event) => { const symbol = event.currentTarget.value; if (symbol !== "") { run({ command: "insert_text", value: symbol }); event.currentTarget.value = ""; } }}><option value="">Symbol</option>{SYMBOLS.map((symbol) => <option key={symbol} value={symbol}>{symbol}</option>)}</NativeSelect></div>
      <OverflowMenu disabled={disabled} label="Insert content" triggerPresentation="text" items={[{ id: "table", label: "Insert table", onSelect: onOpenTable }, { id: "answer-checklist", label: "Insert answer checklist", onSelect: onOpenChecklist }, { id: "divider", label: "Horizontal divider", onSelect: () => run({ command: "insert_horizontal_rule" }) }]} />
      <Button variant="text" size="small" disabled={disabled} onMouseDown={preserveSelection} onClick={() => run({ command: "reset_colors" })}>Reset color</Button>
      <Toolbar label="History and additional formatting" disabled={disabled}>
        <ToolbarGroup label="History and additional formatting">
          {advancedItems.map((item) => <ToolbarAction key={item.id} label={item.label} disabled={item.disabled}
            onMouseDown={preserveSelection} onClick={item.onSelect}>{formatGlyphs[item.id] ?? item.label}</ToolbarAction>)}
          <ToolbarAction label="Clear formatting" onMouseDown={preserveSelection}
            onClick={() => run({ command: "clear_formatting" })}>Tx</ToolbarAction>
        </ToolbarGroup>
      </Toolbar>
    </div>
    </div> : null}
  </fieldset>;
}

const formatGlyphs: Record<string, string> = {
  undo: "↶", redo: "↷", strike: "S̶", "inline-code": "</>", quote: "❝", "code-block": "{ }",
};

function normalizeColor(value: unknown, fallback: string): string { return typeof value === "string" && /^#[0-9a-f]{6}$/iu.test(value) ? value : fallback; }
