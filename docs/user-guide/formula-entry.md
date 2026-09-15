# Enter and edit mathematical formulas

Use the **Formula** action in the rich card editor to insert or edit mathematical notation. oStudy stores validated formula source and renders it locally; you do not need to know LaTeX to use the visual editor.

## Use the graphical controls

The formula dialog opens with a MathLive editing field and an oStudy **Formula tools** panel. Choose a category:

- **Structures** — fractions, powers, roots, subscripts, absolute value, grouping, and a 2 × 2 matrix.
- **Symbols** — common arithmetic and mathematical symbols.
- **Relations** — equality, inequalities, approximation, arrows, and set membership.
- **Greek** — common lowercase and uppercase Greek letters.
- **Calculus** — derivatives, integrals, sums, and limits.

Select an expression in the formula field before pressing a structure button when you want to wrap that expression. For example, select `x² + 7x + 3` and press **Fraction**. MathLive makes the selected expression the numerator and moves the editing position to the denominator placeholder; oStudy does not add parentheses around the numerator.

The same selection behavior works for **Square**, **Power**, **Square root**, **Nth root**, **Subscript**, **Absolute value**, and the grouping controls. When there is no selection, MathLive uses its normal insertion rules and editable placeholders.

Use **Tab** and **Shift+Tab** to move between placeholders. The **Full math keyboard** button opens MathLive's larger virtual keyboard when you need additional symbols or layouts not present in the compact oStudy panel.

## Keyboard shortcuts

The formula dialog's **Shortcuts & help** control contains this reference. These shortcuts follow the pinned MathLive 0.110 editor behavior used by this oStudy release.

| Keys | Action |
| --- | --- |
| `/` | Create a fraction from the selection or preceding expression |
| `^` | Start a superscript/exponent |
| `_` | Start a subscript |
| `Alt+V` | Square root |
| `Alt+Shift+V` | Nth root |
| `Alt+P` | Insert pi |
| `Alt+D` | Insert differential d |
| `Shift+Alt+D` | Insert the partial-derivative symbol |
| `Tab` | Move to the next placeholder |
| `Shift+Tab` | Move to the previous placeholder |
| Arrow keys | Navigate the mathematical structure |
| `Shift` + arrow | Extend the mathematical selection |
| `Ctrl+Z` / `Cmd+Z` | Undo inside the formula field |

Experienced users can also type normal mathematical syntax or LaTeX-style commands directly. That is an optional fast path, not a requirement for formula authoring.

## Placement and saving

**Within the text** creates an inline formula. **On its own line** creates a display formula. The preview shows how the current source will render.

Formula editing is transactional. **Cancel** leaves the card unchanged. **Save formula** applies the captured formula edit as one editor change. Opening and closing the editor without changing the formula does not create a content edit.

Math rendering and formula editing are separate responsibilities: MathLive provides structured input; oStudy's shared KaTeX rendering boundary displays saved formulas in cards, Practice, Tutor, and study guides.
