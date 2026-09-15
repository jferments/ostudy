export type FormulaToolGroupId = "structures" | "symbols" | "relations" | "greek" | "calculus";

export interface FormulaTool {
  id: string;
  label: string;
  preview: string;
  insert: string;
  description: string;
}

export interface FormulaToolGroup {
  id: FormulaToolGroupId;
  label: string;
  tools: readonly FormulaTool[];
}

export interface FormulaShortcut {
  keys: string;
  action: string;
}

export const FORMULA_TOOL_GROUPS: readonly FormulaToolGroup[] = [
  {
    id: "structures",
    label: "Structures",
    tools: [
      { id: "fraction", label: "Fraction", preview: String.raw`\frac{a}{b}`, insert: String.raw`\frac{#@}{#?}`, description: "Put the selection in the numerator and move to the denominator." },
      { id: "square", label: "Square", preview: "x^2", insert: "{#@}^{2}", description: "Square the selected or preceding expression." },
      { id: "power", label: "Power", preview: "x^n", insert: "{#@}^{#?}", description: "Raise the selected or preceding expression to a power." },
      { id: "sqrt", label: "Square root", preview: String.raw`\sqrt{x}`, insert: String.raw`\sqrt{#0}`, description: "Put the selection under a square root." },
      { id: "nth-root", label: "Nth root", preview: String.raw`\sqrt[n]{x}`, insert: String.raw`\sqrt[#?]{#0}`, description: "Put the selection under a root and enter its index." },
      { id: "subscript", label: "Subscript", preview: "x_n", insert: "{#@}_{#?}", description: "Add a subscript to the selected or preceding expression." },
      { id: "absolute", label: "Absolute value", preview: String.raw`\left|x\right|`, insert: String.raw`\left|#0\right|`, description: "Wrap the selection in absolute-value bars." },
      { id: "parentheses", label: "Parentheses", preview: "(x)", insert: String.raw`\left(#0\right)`, description: "Wrap the selection in parentheses." },
      { id: "brackets", label: "Brackets", preview: "[x]", insert: String.raw`\left[#0\right]`, description: "Wrap the selection in square brackets." },
      { id: "braces", label: "Braces", preview: String.raw`\{x\}`, insert: String.raw`\left\{#0\right\}`, description: "Wrap the selection in braces." },
      { id: "matrix", label: "2 by 2 matrix", preview: String.raw`\begin{bmatrix}a&b\\c&d\end{bmatrix}`, insert: String.raw`\begin{bmatrix}#?&#?\\#?&#?\end{bmatrix}`, description: "Insert a two-by-two matrix with editable cells." },
    ],
  },
  {
    id: "symbols",
    label: "Symbols",
    tools: [
      { id: "plus-minus", label: "Plus or minus", preview: String.raw`\pm`, insert: String.raw`\pm`, description: "Insert plus-or-minus." },
      { id: "times", label: "Multiply", preview: String.raw`\times`, insert: String.raw`\times`, description: "Insert a multiplication sign." },
      { id: "dot", label: "Dot product", preview: String.raw`\cdot`, insert: String.raw`\cdot`, description: "Insert a centered multiplication dot." },
      { id: "divide", label: "Divide", preview: String.raw`\div`, insert: String.raw`\div`, description: "Insert a division sign." },
      { id: "infinity", label: "Infinity", preview: String.raw`\infty`, insert: String.raw`\infty`, description: "Insert infinity." },
      { id: "pi", label: "Pi", preview: String.raw`\pi`, insert: String.raw`\pi`, description: "Insert pi." },
      { id: "degree", label: "Degrees", preview: String.raw`30^\circ`, insert: String.raw`^{\circ}`, description: "Insert a degree symbol as a superscript." },
      { id: "partial", label: "Partial", preview: String.raw`\partial`, insert: String.raw`\partial`, description: "Insert the partial-derivative symbol." },
      { id: "nabla", label: "Nabla", preview: String.raw`\nabla`, insert: String.raw`\nabla`, description: "Insert nabla/gradient." },
    ],
  },
  {
    id: "relations",
    label: "Relations",
    tools: [
      { id: "equals", label: "Equals", preview: "=", insert: "=", description: "Insert equals." },
      { id: "not-equal", label: "Not equal", preview: String.raw`\ne`, insert: String.raw`\ne`, description: "Insert not-equal." },
      { id: "less", label: "Less than", preview: "<", insert: "<", description: "Insert less-than." },
      { id: "greater", label: "Greater than", preview: ">", insert: ">", description: "Insert greater-than." },
      { id: "less-equal", label: "Less than or equal", preview: String.raw`\le`, insert: String.raw`\le`, description: "Insert less-than-or-equal." },
      { id: "greater-equal", label: "Greater than or equal", preview: String.raw`\ge`, insert: String.raw`\ge`, description: "Insert greater-than-or-equal." },
      { id: "approximately", label: "Approximately equal", preview: String.raw`\approx`, insert: String.raw`\approx`, description: "Insert approximately-equal." },
      { id: "proportional", label: "Proportional to", preview: String.raw`\propto`, insert: String.raw`\propto`, description: "Insert proportional-to." },
      { id: "right-arrow", label: "Right arrow", preview: String.raw`\to`, insert: String.raw`\to`, description: "Insert a right arrow." },
      { id: "two-way-arrow", label: "Two-way arrow", preview: String.raw`\leftrightarrow`, insert: String.raw`\leftrightarrow`, description: "Insert a two-way arrow." },
      { id: "element", label: "Element of", preview: String.raw`\in`, insert: String.raw`\in`, description: "Insert element-of." },
      { id: "not-element", label: "Not an element of", preview: String.raw`\notin`, insert: String.raw`\notin`, description: "Insert not-an-element-of." },
    ],
  },
  {
    id: "greek",
    label: "Greek",
    tools: [
      { id: "alpha", label: "Alpha", preview: String.raw`\alpha`, insert: String.raw`\alpha`, description: "Insert alpha." },
      { id: "beta", label: "Beta", preview: String.raw`\beta`, insert: String.raw`\beta`, description: "Insert beta." },
      { id: "gamma", label: "Gamma", preview: String.raw`\gamma`, insert: String.raw`\gamma`, description: "Insert gamma." },
      { id: "delta", label: "Delta", preview: String.raw`\delta`, insert: String.raw`\delta`, description: "Insert lowercase delta." },
      { id: "theta", label: "Theta", preview: String.raw`\theta`, insert: String.raw`\theta`, description: "Insert theta." },
      { id: "lambda", label: "Lambda", preview: String.raw`\lambda`, insert: String.raw`\lambda`, description: "Insert lambda." },
      { id: "mu", label: "Mu", preview: String.raw`\mu`, insert: String.raw`\mu`, description: "Insert mu." },
      { id: "rho", label: "Rho", preview: String.raw`\rho`, insert: String.raw`\rho`, description: "Insert rho." },
      { id: "sigma", label: "Sigma", preview: String.raw`\sigma`, insert: String.raw`\sigma`, description: "Insert lowercase sigma." },
      { id: "phi", label: "Phi", preview: String.raw`\phi`, insert: String.raw`\phi`, description: "Insert phi." },
      { id: "omega", label: "Omega", preview: String.raw`\omega`, insert: String.raw`\omega`, description: "Insert lowercase omega." },
      { id: "capital-delta", label: "Capital delta", preview: String.raw`\Delta`, insert: String.raw`\Delta`, description: "Insert capital delta." },
      { id: "capital-sigma", label: "Capital sigma", preview: String.raw`\Sigma`, insert: String.raw`\Sigma`, description: "Insert capital sigma." },
      { id: "capital-omega", label: "Capital omega", preview: String.raw`\Omega`, insert: String.raw`\Omega`, description: "Insert capital omega." },
    ],
  },
  {
    id: "calculus",
    label: "Calculus",
    tools: [
      { id: "derivative", label: "Derivative", preview: String.raw`\frac{d}{dx}f`, insert: String.raw`\frac{d}{d#?}#0`, description: "Insert a derivative template around the selection." },
      { id: "partial-derivative", label: "Partial derivative", preview: String.raw`\frac{\partial f}{\partial x}`, insert: String.raw`\frac{\partial #0}{\partial #?}`, description: "Insert a partial-derivative template." },
      { id: "integral", label: "Integral", preview: String.raw`\int f\,dx`, insert: String.raw`\int #0\,d#?`, description: "Insert an indefinite integral around the selection." },
      { id: "definite-integral", label: "Definite integral", preview: String.raw`\int_a^b f\,dx`, insert: String.raw`\int_{#?}^{#?} #0\,d#?`, description: "Insert an integral with lower and upper limits." },
      { id: "sum", label: "Summation", preview: String.raw`\sum_{i=1}^{n}x_i`, insert: String.raw`\sum_{#?}^{#?} #0`, description: "Insert a summation with lower and upper limits." },
      { id: "limit", label: "Limit", preview: String.raw`\lim_{x\to a}f(x)`, insert: String.raw`\lim_{#?\to #?} #0`, description: "Insert a limit template." },
    ],
  },
] as const;

export const FORMULA_SHORTCUTS: readonly FormulaShortcut[] = [
  { keys: "/", action: "Create a fraction from the current selection or preceding expression" },
  { keys: "^", action: "Start a superscript/exponent" },
  { keys: "_", action: "Start a subscript" },
  { keys: "Alt+V", action: "Square root" },
  { keys: "Alt+Shift+V", action: "Nth root" },
  { keys: "Alt+P", action: "Insert pi" },
  { keys: "Alt+D", action: "Insert differential d" },
  { keys: "Shift+Alt+D", action: "Insert partial-derivative symbol" },
  { keys: "Tab", action: "Move to the next placeholder" },
  { keys: "Shift+Tab", action: "Move to the previous placeholder" },
  { keys: "Arrow keys", action: "Move through the mathematical structure" },
  { keys: "Shift+Arrow", action: "Extend the mathematical selection" },
  { keys: "Ctrl/Cmd+Z", action: "Undo inside the formula field" },
] as const;
