/** Conservative single-dollar inline syntax. No rewriting, TeX evaluation, or
 * global Markdown configuration. Adjacent whitespace and a digit after the
 * closing dollar disqualify a pair, keeping ordinary prices/ranges literal.
 */
export function inlineDollarEnd(text: string): number | undefined {
  if (text[0] !== "$" || text[1] === "$" || !text[1] || /\s/u.test(text[1])) return;
  for (let end = 1; end < text.length; end += 1) {
    // Markdown owns code spans. An unmatched price must not steal a dollar
    // from later code (for example: $10 then `$x$`). Explicit \(…\)
    // delimiters remain available for formulas containing literal backticks.
    if (text[end] === "\n" || text[end] === "\r" || text[end] === "`") return;
    if (text[end] !== "$") continue;
    let slashes = 0;
    for (let i = end - 1; i >= 0 && text[i] === "\\"; i -= 1) slashes += 1;
    if (slashes % 2) continue;
    // Do not skip an invalid closer to swallow subsequent prose or formulas.
    if (end === 1 || /\s/u.test(text[end - 1]) || /[\d$]/u.test(text[end + 1] ?? "")) return;
    return end;
  }
}
