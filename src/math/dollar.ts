/** Conservative single-dollar inline syntax. No rewriting, TeX evaluation, or
 * global Markdown configuration. Adjacent whitespace and a digit after the
 * closing dollar disqualify a pair, keeping ordinary prices/ranges literal.
 */
export function inlineDollarEnd(text: string): number | undefined {
  const second = text.charAt(1);
  if (text.charAt(0) !== "$" || second === "$" || !second || /\s/u.test(second)) return;
  for (let end = 1; end < text.length; end += 1) {
    // Markdown owns code spans. An unmatched price must not steal a dollar
    // from later code (for example: $10 then `$x$`). Explicit \(…\)
    // delimiters remain available for formulas containing literal backticks.
    if (text.charAt(end) === "\n" || text.charAt(end) === "\r" || text.charAt(end) === "`") return;
    if (text.charAt(end) !== "$") continue;
    let slashes = 0;
    for (let i = end - 1; i >= 0 && text.charAt(i) === "\\"; i -= 1) slashes += 1;
    if (slashes % 2) continue;
    // Do not skip an invalid closer to swallow subsequent prose or formulas.
    if (end === 1 || /\s/u.test(text.charAt(end - 1)) || /[\d$]/u.test(text[end + 1] ?? "")) return;
    return end;
  }
}
