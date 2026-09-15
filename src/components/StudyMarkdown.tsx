import type { MarkedToken, Token } from "marked";
import { Fragment, createElement, useMemo, useState, type ReactNode } from "react";

import { openExternalUrl } from "../platform/externalLinks";
import { Formula } from "../math/Formula";
import { isMarkdownFormula, lexStudyMarkdown } from "../math/markdown";
import styles from "./StudyMarkdown.module.css";

const MAX_RENDER_DEPTH = 32;

/** Parse Markdown, never HTML. All generated elements/attributes are owned here.
 * Saved messages use the same renderer as new replies, with no data migration.
 * Image tokens remain alt text: reading a chat must not fetch remote resources.
 */
export function StudyMarkdown({ text, inline = false }: { text: string; inline?: boolean }) {
  const [linkError, setLinkError] = useState(false);
  const tokens = useMemo(() => lexStudyMarkdown(text, inline), [text, inline]);

  function render(items: Token[], depth = 0): ReactNode {
    return items.map((item, index) => <Fragment key={index}>{node(item, depth)}</Fragment>);
  }
  function node(item: Token, depth: number): ReactNode {
    if (depth >= MAX_RENDER_DEPTH) return item.raw;
    if (isMarkdownFormula(item)) return item.complete
      ? <Formula source={item.source} block={item.block} /> : item.raw;
    // Formula tokens are handled above; remaining tokens use the explicit allowlist.
    const token = item as MarkedToken;
    const children = (items: Token[]) => render(items, depth + 1);
    switch (token.type) {
      case "space": case "def": return null;
      case "heading": return createElement(`h${Math.min(6, Math.max(2, token.depth + 1))}`, null, children(token.tokens));
      case "paragraph": return <p>{children(token.tokens)}</p>;
      case "text": return token.tokens ? children(token.tokens) : decodeText(token.text);
      case "escape": return decodeText(token.text);
      case "strong": return <strong>{children(token.tokens)}</strong>;
      case "em": return <em>{children(token.tokens)}</em>;
      case "del": return <del>{children(token.tokens)}</del>;
      case "br": return <br />;
      case "hr": return <hr />;
      case "blockquote": return <blockquote>{children(token.tokens)}</blockquote>;
      case "code": return <pre><code>{token.text}</code></pre>;
      case "codespan": return <code>{token.text}</code>;
      case "html": return token.text;
      case "image": return <span>{decodeText(token.text || "Image")}</span>;
      case "checkbox": return <input type="checkbox" checked={token.checked} disabled aria-label="Checklist item" />;
      case "list_item": return <li>{token.task ? <input type="checkbox" checked={token.checked ?? false} disabled aria-label="Checklist item" /> : null}{children(token.tokens)}</li>;
      case "list": {
        const items = token.items.map((item, index) => <Fragment key={index}>{node(item, depth + 1)}</Fragment>);
        return token.ordered ? <ol start={typeof token.start === "number" ? token.start : 1}>{items}</ol> : <ul>{items}</ul>;
      }
      case "table": return <div className={styles.tableScroll}><table>
        <thead><tr>{token.header.map((cell, index) => <th key={index} scope="col">{children(cell.tokens)}</th>)}</tr></thead>
        <tbody>{token.rows.map((row, index) => <tr key={index}>{row.map((cell, column) => <td key={column}>{children(cell.tokens)}</td>)}</tr>)}</tbody>
      </table></div>;
      case "link": {
        const href = safeExternalHref(token.href);
        if (!href) return children(token.tokens);
        return <a href={href} rel="noreferrer noopener" target="_blank" onClick={(event) => {
          event.preventDefault();
          setLinkError(false);
          void openExternalUrl(href).catch(() => setLinkError(true));
        }}>{children(token.tokens)}</a>;
      }
      default: return item.raw;
    }
  }
  const Host = inline ? "span" : "div";
  return <Host className={styles.markdown}>
    {tokens ? render(tokens) : <span className={styles.plain}>{text}</span>}
    {linkError ? <span role="alert">The link could not be opened.</span> : null}
  </Host>;
}

function safeExternalHref(value: string): string | null {
  const decoded = decodeText(value);
  if (/[\u0000-\u0020\u007f]/u.test(decoded)) return null;
  try {
    const url = new URL(decoded);
    if (!["https:", "http:", "mailto:"].includes(url.protocol) || url.username || url.password) return null;
    return url.href;
  } catch { return null; }
}

/** Decode text entities without creating an HTML document or assigning innerHTML.
 * Unrecognized named entities stay readable rather than being guessed.
 */
function decodeText(value: string): string {
  const named: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: "\u00a0" };
  return value.replace(/&(#x[0-9a-f]+|#[0-9]+|amp|lt|gt|quot|apos|nbsp);/giu, (entity, code: string) => {
    if (!code.startsWith("#")) return named[code.toLowerCase()] ?? entity;
    const hex = code[1].toLowerCase() === "x";
    const number = Number.parseInt(code.slice(hex ? 2 : 1), hex ? 16 : 10);
    return number > 0 && number <= 0x10ffff && !(number >= 0xd800 && number <= 0xdfff)
      ? String.fromCodePoint(number) : "\ufffd";
  });
}
