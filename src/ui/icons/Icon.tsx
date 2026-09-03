import type { ReactNode, SVGAttributes } from "react";

export const ICON_NAMES = [
  "archive",
  "analytics",
  "back",
  "book",
  "brain",
  "calendar",
  "cards",
  "check",
  "chevron-down",
  "chevron-right",
  "close",
  "copy",
  "delete",
  "document",
  "download",
  "edit",
  "export",
  "filter",
  "folder",
  "folder-open",
  "full-screen",
  "help",
  "import",
  "information",
  "maximize",
  "menu",
  "move",
  "overflow",
  "plus",
  "practice",
  "restore",
  "schedule",
  "search",
  "settings",
  "sort",
  "sparkles",
  "stop",
  "study-guide",
  "template",
  "timer",
  "upload",
  "warning",
] as const;

export type IconName = (typeof ICON_NAMES)[number];

export interface IconProps extends SVGAttributes<SVGSVGElement> {
  name: IconName;
  size?: number;
  label?: string;
}

export function Icon({ name, size = 20, label, ...props }: IconProps) {
  const resolvedSize = Number.isFinite(size) ? Math.max(12, Math.min(64, size)) : 20;
  return (
    <svg
      {...props}
      width={resolvedSize}
      height={resolvedSize}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
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
    case "archive":
      return (
        <>
          <path d="M4 7h16v13H4z" />
          <path d="M3 3h18v4H3zM9 11h6" />
        </>
      );
    case "analytics":
      return <path d="M4 20V10M10 20V4M16 20v-7M22 20H2" />;
    case "back":
      return (
        <>
          <path d="m15 18-6-6 6-6" />
          <path d="M9 12h11" />
        </>
      );
    case "book":
    case "study-guide":
      return (
        <>
          <path d="M4 5.5A3.5 3.5 0 0 1 7.5 2H12v18H7.5A3.5 3.5 0 0 0 4 23z" />
          <path d="M20 5.5A3.5 3.5 0 0 0 16.5 2H12v18h4.5A3.5 3.5 0 0 1 20 23z" />
        </>
      );
    case "brain":
      return (
        <>
          <path d="M9.5 4.6A3.2 3.2 0 0 0 4 6.8a3.4 3.4 0 0 0-.2 5.7A3.5 3.5 0 0 0 7 18.2 3 3 0 0 0 12 20V6.2a3 3 0 0 0-2.5-1.6Z" />
          <path d="M14.5 4.6A3.2 3.2 0 0 1 20 6.8a3.4 3.4 0 0 1 .2 5.7 3.5 3.5 0 0 1-3.2 5.7A3 3 0 0 1 12 20V6.2a3 3 0 0 1 2.5-1.6Z" />
          <path d="M7.5 9.5c1.5 0 2.5 1 2.5 2.5M16.5 9.5c-1.5 0-2.5 1-2.5 2.5M8 15c1.2 0 2 .7 2 1.8M16 15c-1.2 0-2 .7-2 1.8" />
        </>
      );
    case "calendar":
    case "schedule":
      return (
        <>
          <rect x="3" y="5" width="18" height="16" rx="2" />
          <path d="M7 3v4M17 3v4M3 10h18" />
          <path d="M8 14h3M8 17h3M14 14h2" />
        </>
      );
    case "cards":
      return (
        <>
          <rect x="3" y="5" width="15" height="14" rx="2" />
          <path d="M7 2h12a2 2 0 0 1 2 2v11" />
        </>
      );
    case "check":
      return <path d="m5 12 4 4L19 6" />;
    case "chevron-down":
      return <path d="m6 9 6 6 6-6" />;
    case "chevron-right":
      return <path d="m9 6 6 6-6 6" />;
    case "close":
      return <path d="m6 6 12 12M18 6 6 18" />;
    case "copy":
      return (
        <>
          <rect x="8" y="8" width="12" height="12" rx="2" />
          <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
        </>
      );
    case "delete":
      return <path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5" />;
    case "document":
      return (
        <>
          <path d="M6 2h8l4 4v16H6z" />
          <path d="M14 2v5h5M9 12h6M9 16h6" />
        </>
      );
    case "download":
      return (
        <>
          <path d="M12 3v12m0 0 4-4m-4 4-4-4" />
          <path d="M4 19h16" />
        </>
      );
    case "edit":
      return (
        <>
          <path d="m4 20 4-1 11-11-3-3L5 16z" />
          <path d="m14 7 3 3" />
        </>
      );
    case "export":
      return (
        <>
          <path d="M14 4h6v6M20 4l-9 9" />
          <path d="M18 13v7H4V6h7" />
        </>
      );
    case "filter":
      return <path d="M3 5h18l-7 8v6l-4 2v-8z" />;
    case "folder":
      return <path d="M3 6h7l2 2h9v11H3z" />;
    case "folder-open":
      return (
        <>
          <path d="M3 7h7l2 2h9l-3 10H3z" />
          <path d="M3 7v12" />
        </>
      );
    case "full-screen":
    case "maximize":
      return (
        <>
          <path d="M9 4H4v5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
          <path d="m4 9 5-5M15 4l5 5M20 15l-5 5M9 20l-5-5" />
        </>
      );
    case "help":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M9.8 9a2.4 2.4 0 1 1 3.2 2.3c-.8.4-1 1-1 1.7M12 17h.01" />
        </>
      );
    case "import":
      return (
        <>
          <path d="M10 4H4v16h14v-7" />
          <path d="M20 4h-7m7 0v7m0-7-9 9" />
        </>
      );
    case "information":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 11v6M12 7h.01" />
        </>
      );
    case "menu":
      return <path d="M4 7h16M4 12h16M4 17h16" />;
    case "move":
      return (
        <>
          <path d="M12 2v20M2 12h20" />
          <path d="m8 6 4-4 4 4M8 18l4 4 4-4M6 8l-4 4 4 4M18 8l4 4-4 4" />
        </>
      );
    case "overflow":
      return (
        <>
          <circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" />
        </>
      );
    case "plus":
      return <path d="M12 5v14M5 12h14" />;
    case "practice":
      return (
        <>
          <path d="m8 5 11 7-11 7z" />
          <path d="M4 5v14" />
        </>
      );
    case "restore":
      return (
        <>
          <path d="M4 7v5h5" />
          <path d="M5.5 16a8 8 0 1 0 .5-9l-2 5" />
        </>
      );
    case "search":
      return (
        <>
          <circle cx="10.5" cy="10.5" r="6.5" />
          <path d="m16 16 5 5" />
        </>
      );
    case "settings":
      return (
        <>
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19 13.5v-3l-2-.7-.8-1.8.9-1.9-2.2-2.2-1.9.9-1.8-.8L10.5 1h-3L6.8 3l-1.8.8-1.9-.9L.9 5.1 1.8 7 1 8.8l-2 .7v3l2 .7.8 1.8-.9 1.9 2.2 2.2 1.9-.9 1.8.8.7 2h3l.7-2 1.8-.8 1.9.9 2.2-2.2-.9-1.9.8-1.8z"
            transform="translate(2) scale(.83)"
          />
        </>
      );
    case "sort":
      return (
        <>
          <path d="M8 6h12M8 12h8M8 18h4" />
          <path d="m3 5 2-2 2 2M5 3v16m-2-2 2 2 2-2" />
        </>
      );
    case "sparkles":
      return (
        <>
          <path d="m12 3 1.4 4.1 4.1 1.4-4.1 1.4L12 14l-1.4-4.1-4.1-1.4 4.1-1.4z" />
          <path d="m19 14 .8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8zM5 15l.7 1.8 1.8.7-1.8.7L5 20l-.7-1.8-1.8-.7 1.8-.7z" />
        </>
      );
    case "stop":
      return (
        <>
          <path d="m8 3h8l5 5v8l-5 5H8l-5-5V8z" />
          <rect x="9" y="9" width="6" height="6" rx="0.8" fill="currentColor" stroke="none" />
        </>
      );
    case "template":
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M3 9h18M9 9v12" />
        </>
      );
    case "timer":
      return (
        <>
          <circle cx="12" cy="13" r="8" />
          <path d="M12 9v4l3 2M9 2h6M12 2v3M5 4 3 6M19 4l2 2" />
        </>
      );
    case "upload":
      return (
        <>
          <path d="M12 21V9m0 0 4 4m-4-4-4 4" />
          <path d="M4 5h16" />
        </>
      );
    case "warning":
      return (
        <>
          <path d="M12 3 2 21h20z" />
          <path d="M12 9v5M12 18h.01" />
        </>
      );
  }

  const unhandledIcon: never = name;
  return unhandledIcon;
}
