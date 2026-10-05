import { useMemo, useState } from "react";

import { Formula } from "../math/Formula";
import {
  FORMULA_SHORTCUTS,
  FORMULA_TOOL_GROUPS,
  type FormulaToolGroupId,
} from "../math/formulaTools";
import type { FormulaInput } from "../math/mathlive";
import { Button } from "../ui/primitives/Button";

export function FormulaToolbox({ input, disabled, showKeyboardHelp = true }: {
  input: FormulaInput | null;
  disabled: boolean;
  showKeyboardHelp?: boolean;
}) {
  const [activeGroup, setActiveGroup] = useState<FormulaToolGroupId>("structures");
  const [showHelp, setShowHelp] = useState(false);
  const group = useMemo(
    () => FORMULA_TOOL_GROUPS.find((candidate) => candidate.id === activeGroup) ?? FORMULA_TOOL_GROUPS[0],
    [activeGroup],
  );
  if (!group) throw new Error("Formula tools require at least one configured group.");

  function insert(template: string): void {
    input?.applyTemplate(template);
  }

  return (
    <section
      aria-label="Visual formula controls"
      className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted/20 tw:p-3"
    >
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3">
        <div className="tw:min-w-0">
          <strong className="tw:text-small">Formula tools</strong>
          <p className="tw:mt-1 tw:mb-0 tw:max-w-3xl tw:text-caption tw:leading-relaxed tw:text-muted-foreground">
            Select an existing expression before choosing a structure to wrap it. Fraction, powers, roots, and brackets preserve the selection without adding extra parentheses.
          </p>
        </div>
        {showKeyboardHelp ? (
          <Button
            type="button"
            size="small"
            variant="ghost"
            aria-expanded={showHelp}
            aria-controls="formula-shortcuts-help"
            onClick={() => setShowHelp((value) => !value)}
          >
            {showHelp ? "Hide help" : "Shortcuts & help"}
          </Button>
        ) : null}
      </div>

      <div
        role="tablist"
        aria-label="Formula tool categories"
        className="tw:flex tw:min-w-0 tw:flex-wrap tw:gap-1.5"
      >
        {FORMULA_TOOL_GROUPS.map((candidate) => (
          <Button
            key={candidate.id}
            id={`formula-tools-tab-${candidate.id}`}
            type="button"
            size="small"
            variant={activeGroup === candidate.id ? "primary" : "secondary"}
            role="tab"
            aria-selected={activeGroup === candidate.id}
            aria-controls={`formula-tools-panel-${candidate.id}`}
            disabled={disabled}
            onClick={() => setActiveGroup(candidate.id)}
          >
            {candidate.label}
          </Button>
        ))}
      </div>

      <div
        id={`formula-tools-panel-${group.id}`}
        role="tabpanel"
        aria-labelledby={`formula-tools-tab-${group.id}`}
        className="tw:grid tw:min-w-0 tw:grid-cols-[repeat(auto-fit,minmax(5.75rem,1fr))] tw:gap-2"
      >
        {group.tools.map((tool) => (
          <Button
            key={tool.id}
            type="button"
            variant="secondary"
            disabled={disabled || !input}
            aria-label={tool.label}
            title={`${tool.label}: ${tool.description}`}
            className="tw:min-h-12 tw:min-w-0 tw:px-2 tw:py-2 tw:[&>span]:min-w-0"
            // Keep the MathLive selection active for pointer users so #@/#0
            // can wrap it. Keyboard users retain MathLive's internal selection.
            onPointerDown={(event) => event.preventDefault()}
            onClick={() => insert(tool.insert)}
          >
            <span aria-hidden="true" className="tw:flex tw:min-w-0 tw:items-center tw:justify-center tw:text-body">
              <Formula source={tool.preview} block={false} />
            </span>
          </Button>
        ))}
      </div>

      {showKeyboardHelp && showHelp ? (
        <div
          id="formula-shortcuts-help"
          className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-md tw:bg-background tw:p-3 tw:ring-1 tw:ring-border"
        >
          <div>
            <strong className="tw:text-small">Common operations</strong>
            <p className="tw:mt-1 tw:mb-0 tw:text-small tw:leading-relaxed">
              Highlight an expression, then choose <strong>Fraction</strong> to make it the numerator, <strong>Square</strong> or <strong>Power</strong> to add an exponent, or <strong>Square root</strong> to put it under a radical. Empty boxes are placeholders; use Tab and Shift+Tab to move between them.
            </p>
          </div>
          <div>
            <strong className="tw:text-small">Keyboard shortcuts</strong>
            <div className="tw:mt-2 tw:grid tw:grid-cols-[max-content_minmax(0,1fr)] tw:gap-x-3 tw:gap-y-1.5 tw:text-small">
              {FORMULA_SHORTCUTS.map((shortcut) => (
                <div key={shortcut.keys} className="tw:contents">
                  <kbd className="tw:rounded tw:border tw:border-solid tw:border-border tw:bg-muted tw:px-1.5 tw:py-0.5 tw:font-mono tw:text-caption">
                    {shortcut.keys}
                  </kbd>
                  <span>{shortcut.action}</span>
                </div>
              ))}
            </div>
          </div>
          <p className="tw:m-0 tw:text-caption tw:leading-relaxed tw:text-muted-foreground">
            You can also type LaTeX-style commands if you know them, but they are optional. The graphical controls and full MathLive keyboard are the intended path for users who do not know formula syntax.
          </p>
        </div>
      ) : null}
    </section>
  );
}
