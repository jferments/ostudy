import type { CSSProperties, ReactNode, SVGAttributes } from "react";

/* Glyphs are Lucide icons (https://lucide.dev, lucide-static 1.52.0), vendored as path data
 * so the app ships no icon package. Lucide is ISC licensed and derives from Feather (MIT); the
 * distributed notice is public/licenses/LUCIDE-ISC.txt. Each case names its Lucide source icon. */

export const ICON_NAMES = [
  "analytics",
  "annotate",
  "archive",
  "arrow-down",
  "arrow-right",
  "arrow-up",
  "back",
  "book",
  "brain",
  "calendar",
  "calendar-check",
  "cards",
  "chat",
  "check",
  "chevron-down",
  "chevron-left",
  "chevron-right",
  "close",
  "contents",
  "copy",
  "delete",
  "document",
  "download",
  "edit",
  "eraser",
  "export",
  "filter",
  "folder",
  "folder-open",
  "formula",
  "full-screen",
  "help",
  "highlighter",
  "home",
  "image",
  "import",
  "information",
  "keyboard",
  "library",
  "lightbulb",
  "link",
  "maximize",
  "media",
  "menu",
  "minus",
  "monitor",
  "move",
  "overflow",
  "pan",
  "pen",
  "plus",
  "pointer",
  "practice",
  "redo",
  "restore",
  "save",
  "scan",
  "schedule",
  "search",
  "settings",
  "shape-arrow",
  "shape-ellipse",
  "shape-line",
  "shape-rectangle",
  "smartphone",
  "sort",
  "sparkles",
  "sticky-note",
  "stop",
  "study-guide",
  "template",
  "text",
  "timer",
  "undo",
  "upload",
  "usb",
  "view",
  "view-off",
  "warning",
  "wifi",
  "zoom-in",
  "zoom-out",
] as const;

export type IconName = (typeof ICON_NAMES)[number];

export interface IconProps extends SVGAttributes<SVGSVGElement> {
  name: IconName;
  size?: number;
  label?: string;
}

export function Icon({ name, size = 20, label, style, ...props }: IconProps) {
  const resolvedSize = Number.isFinite(size) ? Math.max(12, Math.min(64, size)) : 20;
  // The attributes keep the design size; the icon default in document.css restates it in
  // rem so the interface scale reaches icons, while size utilities on the svg or its
  // button still win.
  const scaledStyle = { ...style, "--icon-size": `${resolvedSize / 16}rem` } as CSSProperties;
  return (
    <svg
      data-icon={name}
      {...props}
      style={scaledStyle}
      width={resolvedSize}
      height={resolvedSize}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.9"
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : "true"}
      focusable="false"
    >
      {iconPath(name)}
    </svg>
  );
}

function iconPath(name: IconName): ReactNode {
  switch (name) {
    case "analytics": // Lucide: chart-column
      return (
        <>
          <path d="M3 3v16a2 2 0 0 0 2 2h16" />
          <path d="M18 17V9" />
          <path d="M13 17V5" />
          <path d="M8 17v-3" />
        </>
      );
    case "annotate": // Lucide: pen-line
      return (
        <>
          <path d="M13 21h8" />
          <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" />
        </>
      );
    case "archive": // Lucide: archive
      return (
        <>
          <rect x="2" y="3" width="20" height="5" rx="1" />
          <path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" />
          <path d="M10 12h4" />
        </>
      );
    case "arrow-down": // Lucide: arrow-down
      return (
        <>
          <path d="M12 5v14" />
          <path d="m19 12-7 7-7-7" />
        </>
      );
    case "arrow-right": // Lucide: arrow-right
      return (
        <>
          <path d="M5 12h14" />
          <path d="m12 5 7 7-7 7" />
        </>
      );
    case "arrow-up": // Lucide: arrow-up
      return (
        <>
          <path d="m5 12 7-7 7 7" />
          <path d="M12 19V5" />
        </>
      );
    case "back": // Lucide: arrow-left
      return (
        <>
          <path d="m12 19-7-7 7-7" />
          <path d="M19 12H5" />
        </>
      );
    case "book": // Lucide: book-open
      return (
        <>
          <path d="M12 5v16" />
          <path d="M20.001 19A2 2 0 0022 17V5a2 2 0 00-1.999-2L16 3.002A5 5 0 0012 5a5 5 0 00-4-2H4a2 2 0 00-2 2v12a2 2 0 001.999 2H8a5 5 0 014 2 5 5 0 014-2z" />
        </>
      );
    case "brain": // Lucide: brain
      return (
        <>
          <path d="M12 18V5" />
          <path d="M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
          <path d="M17.598 6.5A3 3 0 1 0 12 5a3 3 0 1 0-5.598 1.5" />
          <path d="M17.997 5.125a4 4 0 0 1 2.526 5.77" />
          <path d="M18 18a4 4 0 0 0 2-7.464" />
          <path d="M19.967 17.483A4 4 0 1 1 12 18a4 4 0 1 1-7.967-.517" />
          <path d="M6 18a4 4 0 0 1-2-7.464" />
          <path d="M6.003 5.125a4 4 0 0 0-2.526 5.77" />
        </>
      );
    case "calendar": // Lucide: calendar
      return (
        <>
          <path d="M8 2v3" />
          <path d="M16 2v3" />
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M3 9h18" />
        </>
      );
    case "calendar-check": // Lucide: calendar-check
      return (
        <>
          <path d="M8 2v3" />
          <path d="M16 2v3" />
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M3 9h18" />
          <path d="m9 15 2 2 4-4" />
        </>
      );
    case "cards": // Lucide: layers
      return (
        <>
          <path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z" />
          <path d="M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12" />
          <path d="M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17" />
        </>
      );
    case "chat": // Lucide: message-square-text
      return (
        <>
          <path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z" />
          <path d="M7 11h10" />
          <path d="M7 15h6" />
          <path d="M7 7h8" />
        </>
      );
    case "check": // Lucide: check
      return <path d="M20 6 9 17l-5-5" />;
    case "chevron-down": // Lucide: chevron-down
      return <path d="m6 9 6 6 6-6" />;
    case "chevron-left": // Lucide: chevron-left
      return <path d="m15 18-6-6 6-6" />;
    case "chevron-right": // Lucide: chevron-right
      return <path d="m9 18 6-6-6-6" />;
    case "close": // Lucide: x
      return (
        <>
          <path d="M18 6 6 18" />
          <path d="m6 6 12 12" />
        </>
      );
    case "contents": // Lucide: list
      return (
        <>
          <path d="M3 5h.01" />
          <path d="M3 12h.01" />
          <path d="M3 19h.01" />
          <path d="M8 5h13" />
          <path d="M8 12h13" />
          <path d="M8 19h13" />
        </>
      );
    case "copy": // Lucide: copy
      return (
        <>
          <rect x="8" y="8" width="14" height="14" rx="2" ry="2" />
          <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
        </>
      );
    case "delete": // Lucide: trash
      return (
        <>
          <path d="M10 11v6" />
          <path d="M14 11v6" />
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
          <path d="M3 6h18" />
          <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
        </>
      );
    case "document": // Lucide: file-text
      return (
        <>
          <path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z" />
          <path d="M14 2v5a1 1 0 0 0 1 1h5" />
          <path d="M10 9H8" />
          <path d="M16 13H8" />
          <path d="M16 17H8" />
        </>
      );
    case "download": // Lucide: download
      return (
        <>
          <path d="M12 15V3" />
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <path d="m7 10 5 5 5-5" />
        </>
      );
    case "edit": // Lucide: pencil
      return (
        <>
          <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" />
          <path d="m15 5 4 4" />
        </>
      );
    case "eraser": // Lucide: eraser
      return (
        <>
          <path d="M21 21H8a2 2 0 0 1-1.42-.587l-3.994-3.999a2 2 0 0 1 0-2.828l10-10a2 2 0 0 1 2.829 0l5.999 6a2 2 0 0 1 0 2.828L12.834 21" />
          <path d="m5.082 11.09 8.828 8.828" />
        </>
      );
    case "export": // Lucide: share
      return (
        <>
          <path d="M12 2v13" />
          <path d="m16 6-4-4-4 4" />
          <path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8" />
        </>
      );
    case "filter": // Lucide: funnel
      return <path d="M10 20a1 1 0 0 0 .553.895l2 1A1 1 0 0 0 14 21v-7a2 2 0 0 1 .517-1.341L21.74 4.67A1 1 0 0 0 21 3H3a1 1 0 0 0-.742 1.67l7.225 7.989A2 2 0 0 1 10 14z" />;
    case "folder": // Lucide: folder
      return <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />;
    case "folder-open": // Lucide: folder-open
      return <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />;
    case "formula": // Lucide: square-function
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
          <path d="M9 17c2 0 2.8-1 2.8-2.8V10c0-2 1-3.3 3.2-3" />
          <path d="M9 11.2h5.7" />
        </>
      );
    case "full-screen": // Lucide: fullscreen
      return (
        <>
          <path d="M3 7V5a2 2 0 0 1 2-2h2" />
          <path d="M17 3h2a2 2 0 0 1 2 2v2" />
          <path d="M21 17v2a2 2 0 0 1-2 2h-2" />
          <path d="M7 21H5a2 2 0 0 1-2-2v-2" />
          <rect x="7" y="8" width="10" height="8" rx="1" />
        </>
      );
    case "help": // Lucide: circle-question-mark
      return (
        <>
          <circle cx="12" cy="12" r="10" />
          <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
          <path d="M12 17h.01" />
        </>
      );
    case "highlighter": // Lucide: highlighter
      return (
        <>
          <path d="m9 11-6 6v3h9l3-3" />
          <path d="m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4" />
        </>
      );
    case "home": // Lucide: house
      return (
        <>
          <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" />
          <path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </>
      );
    case "monitor": // Lucide: monitor
      return (
        <>
          <rect width="20" height="14" x="2" y="3" rx="2" />
          <path d="M8 21h8" />
          <path d="M12 17v4" />
        </>
      );
    case "smartphone": // Lucide: smartphone
      return (
        <>
          <rect width="14" height="20" x="5" y="2" rx="2" ry="2" />
          <path d="M12 18h.01" />
        </>
      );
    case "usb": // Lucide: usb
      return (
        <>
          <circle cx="10" cy="7" r="1" />
          <circle cx="4" cy="20" r="1" />
          <path d="M4.7 19.3 19 5" />
          <path d="m21 3-3 1 2 2Z" />
          <path d="M9.26 7.68 5 12l2 5" />
          <path d="m10 14 5 2 3.5-3.5" />
          <path d="m18 12 1-1 1 1-1 1Z" />
        </>
      );
    case "wifi": // Lucide: wifi
      return (
        <>
          <path d="M12 20h.01" />
          <path d="M2 8.82a15 15 0 0 1 20 0" />
          <path d="M5 12.859a10 10 0 0 1 14 0" />
          <path d="M8.5 16.429a5 5 0 0 1 7 0" />
        </>
      );
    case "scan": // Lucide: scan-line
      return (
        <>
          <path d="M3 7V5a2 2 0 0 1 2-2h2" />
          <path d="M17 3h2a2 2 0 0 1 2 2v2" />
          <path d="M21 17v2a2 2 0 0 1-2 2h-2" />
          <path d="M7 21H5a2 2 0 0 1-2-2v-2" />
          <path d="M7 12h10" />
        </>
      );
    case "image": // Lucide: image
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
          <circle cx="9" cy="9" r="2" />
          <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
        </>
      );
    case "import": // Lucide: import
      return (
        <>
          <path d="M12 3v12" />
          <path d="m8 11 4 4 4-4" />
          <path d="M8 5H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-4" />
        </>
      );
    case "information": // Lucide: info
      return (
        <>
          <circle cx="12" cy="12" r="10" />
          <path d="M12 16v-4" />
          <path d="M12 8h.01" />
        </>
      );
    case "keyboard": // Lucide: keyboard
      return (
        <>
          <path d="M10 8h.01" />
          <path d="M12 12h.01" />
          <path d="M14 8h.01" />
          <path d="M16 12h.01" />
          <path d="M18 8h.01" />
          <path d="M6 8h.01" />
          <path d="M7 16h10" />
          <path d="M8 12h.01" />
          <rect x="2" y="4" width="20" height="16" rx="2" />
        </>
      );
    case "library": // Lucide: layers-2
      return (
        <>
          <path d="M13 13.74a2 2 0 0 1-2 0L2.5 8.87a1 1 0 0 1 0-1.74L11 2.26a2 2 0 0 1 2 0l8.5 4.87a1 1 0 0 1 0 1.74z" />
          <path d="m20 14.285 1.5.845a1 1 0 0 1 0 1.74L13 21.74a2 2 0 0 1-2 0l-8.5-4.87a1 1 0 0 1 0-1.74l1.5-.845" />
        </>
      );
    case "lightbulb": // Lucide: lightbulb
      return (
        <>
          <path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5" />
          <path d="M9 18h6" />
          <path d="M10 22h4" />
        </>
      );
    case "link": // Lucide: link
      return (
        <>
          <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
          <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
        </>
      );
    case "maximize": // Lucide: maximize
      return (
        <>
          <path d="M8 3H5a2 2 0 0 0-2 2v3" />
          <path d="M21 8V5a2 2 0 0 0-2-2h-3" />
          <path d="M3 16v3a2 2 0 0 0 2 2h3" />
          <path d="M16 21h3a2 2 0 0 0 2-2v-3" />
        </>
      );
    case "media": // Lucide: square-play
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M9 9.003a1 1 0 0 1 1.517-.859l4.997 2.997a1 1 0 0 1 0 1.718l-4.997 2.997A1 1 0 0 1 9 14.996z" />
        </>
      );
    case "menu": // Lucide: menu
      return (
        <>
          <path d="M4 5h16" />
          <path d="M4 12h16" />
          <path d="M4 19h16" />
        </>
      );
    case "minus": // Lucide: minus
      return <path d="M5 12h14" />;
    case "move": // Lucide: move
      return (
        <>
          <path d="M12 2v20" />
          <path d="m15 19-3 3-3-3" />
          <path d="m19 9 3 3-3 3" />
          <path d="M2 12h20" />
          <path d="m5 9-3 3 3 3" />
          <path d="m9 5 3-3 3 3" />
        </>
      );
    case "overflow": // Lucide: ellipsis
      return (
        <>
          <circle cx="12" cy="12" r="1" />
          <circle cx="19" cy="12" r="1" />
          <circle cx="5" cy="12" r="1" />
        </>
      );
    case "pan": // Lucide: hand
      return (
        <>
          <path d="M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2" />
          <path d="M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2" />
          <path d="M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8" />
          <path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15" />
        </>
      );
    case "pen": // Lucide: pen
      return <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" />;
    case "plus": // Lucide: plus
      return (
        <>
          <path d="M5 12h14" />
          <path d="M12 5v14" />
        </>
      );
    case "pointer": // Lucide: mouse-pointer-2
      return <path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z" />;
    case "practice": // Lucide: play
      return <path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" />;
    case "redo": // Lucide: redo-2
      return (
        <>
          <path d="m15 14 5-5-5-5" />
          <path d="M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13" />
        </>
      );
    case "restore": // Lucide: rotate-ccw
      return (
        <>
          <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
          <path d="M3 3v5h5" />
        </>
      );
    case "save": // Lucide: save
      return (
        <>
          <path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
          <path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" />
          <path d="M7 3v4a1 1 0 0 0 1 1h7" />
        </>
      );
    case "schedule": // Lucide: calendar-clock
      return (
        <>
          <path d="M16 14v2.2l1.6 1" />
          <path d="M16 2v3" />
          <path d="M21 7.338V5a2 2 0 00-2-2H5a2 2 0 00-2 2v14a2 2 0 002 2h2.338" />
          <path d="M3 9h5.859" />
          <path d="M8 2v3" />
          <circle cx="16" cy="16" r="6" />
        </>
      );
    case "search": // Lucide: search
      return (
        <>
          <path d="m21 21-4.34-4.34" />
          <circle cx="11" cy="11" r="8" />
        </>
      );
    case "settings": // Lucide: settings
      return (
        <>
          <path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" />
          <circle cx="12" cy="12" r="3" />
        </>
      );
    case "shape-arrow": // Lucide: move-up-right
      return (
        <>
          <path d="M13 5H19V11" />
          <path d="M19 5L5 19" />
        </>
      );
    case "shape-ellipse": // Lucide: circle
      return <circle cx="12" cy="12" r="10" />;
    case "shape-line": // Lucide: slash
      return <path d="M22 2 2 22" />;
    case "shape-rectangle": // Lucide: square
      return <rect width="18" height="18" x="3" y="3" rx="2" />;
    case "sort": // Lucide: arrow-down-wide-narrow
      return (
        <>
          <path d="m3 16 4 4 4-4" />
          <path d="M7 20V4" />
          <path d="M11 4h10" />
          <path d="M11 8h7" />
          <path d="M11 12h4" />
        </>
      );
    case "sparkles": // Lucide: sparkles
      return (
        <>
          <path d="M11.017 2.814a1 1 0 0 1 1.966 0l1.051 5.558a2 2 0 0 0 1.594 1.594l5.558 1.051a1 1 0 0 1 0 1.966l-5.558 1.051a2 2 0 0 0-1.594 1.594l-1.051 5.558a1 1 0 0 1-1.966 0l-1.051-5.558a2 2 0 0 0-1.594-1.594l-5.558-1.051a1 1 0 0 1 0-1.966l5.558-1.051a2 2 0 0 0 1.594-1.594z" />
          <path d="M20 2v4" />
          <path d="M22 4h-4" />
          <circle cx="4" cy="20" r="2" />
        </>
      );
    case "sticky-note": // Lucide: sticky-note
      return (
        <>
          <path d="M21 9a2.4 2.4 0 0 0-.706-1.706l-3.588-3.588A2.4 2.4 0 0 0 15 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2z" />
          <path d="M15 3v5a1 1 0 0 0 1 1h5" />
        </>
      );
    case "stop": // Lucide: circle-stop
      return (
        <>
          <circle cx="12" cy="12" r="10" />
          <rect x="9" y="9" width="6" height="6" rx="1" />
        </>
      );
    case "study-guide": // Lucide: book-open-text
      return (
        <>
          <path d="M12 5v16" />
          <path d="M16 13h2" />
          <path d="M16 9h2" />
          <path d="M20.001 19A2 2 0 0022 17V5a2 2 0 00-1.999-2L16 3.002A5 5 0 0012 5a5 5 0 00-4-2H4a2 2 0 00-2 2v12a2 2 0 001.999 2H8a5 5 0 014 2 5 5 0 014-2z" />
          <path d="M6 13h2" />
          <path d="M6 9h2" />
        </>
      );
    case "template": // Lucide: panels-top-left
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M3 9h18" />
          <path d="M9 21V9" />
        </>
      );
    case "text": // Lucide: type
      return (
        <>
          <path d="M12 4v16" />
          <path d="M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2" />
          <path d="M9 20h6" />
        </>
      );
    case "timer": // Lucide: timer
      return (
        <>
          <line x1="10" y1="2" x2="14" y2="2" />
          <line x1="12" y1="14" x2="15" y2="11" />
          <circle cx="12" cy="14" r="8" />
        </>
      );
    case "undo": // Lucide: undo-2
      return (
        <>
          <path d="M9 14 4 9l5-5" />
          <path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11" />
        </>
      );
    case "upload": // Lucide: upload
      return (
        <>
          <path d="M12 3v12" />
          <path d="m17 8-5-5-5 5" />
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
        </>
      );
    case "view": // Lucide: eye
      return (
        <>
          <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" />
          <circle cx="12" cy="12" r="3" />
        </>
      );
    case "view-off": // Lucide: eye-off
      return (
        <>
          <path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" />
          <path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" />
          <path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" />
          <path d="m2 2 20 20" />
        </>
      );
    case "warning": // Lucide: triangle-alert
      return (
        <>
          <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
          <path d="M12 9v4" />
          <path d="M12 17h.01" />
        </>
      );
    case "zoom-in": // Lucide: zoom-in
      return (
        <>
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
          <line x1="11" y1="8" x2="11" y2="14" />
          <line x1="8" y1="11" x2="14" y2="11" />
        </>
      );
    case "zoom-out": // Lucide: zoom-out
      return (
        <>
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
          <line x1="8" y1="11" x2="14" y2="11" />
        </>
      );
  }

  const unhandledIcon: never = name;
  return unhandledIcon;
}
