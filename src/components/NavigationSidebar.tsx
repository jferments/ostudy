import { useState } from "react";

import type { AppSection } from "../controllers/useAppController";
import { useMediaQuery } from "../ui/hooks/useMediaQuery";
import { Sheet } from "../ui/overlays/Dialog";
import styles from "./NavigationSidebar.module.css";

interface NavigationSidebarProps {
  section: AppSection;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

const ITEMS: ReadonlyArray<{ section: AppSection; icon: string; label: string }> = [
  { section: "decks", icon: "▤", label: "Decks" },
  { section: "documents", icon: "▣", label: "Documents" },
  { section: "practice", icon: "▶", label: "Practice" },
  { section: "analytics", icon: "▥", label: "Analytics" },
  { section: "settings", icon: "⚙", label: "Settings" },
];

const COMPACT_SECTIONS = new Set<AppSection>(["decks", "practice", "analytics"]);

function isActive(item: AppSection, current: AppSection): boolean {
  if (item === "decks") {
    return current === "decks" || current === "cards" || current === "templates";
  }
  return item === current;
}

export function NavigationSidebar({ section, onNavigate }: NavigationSidebarProps) {
  const compact = useMediaQuery("(max-width: 760px)");
  const [moreOpen, setMoreOpen] = useState(false);

  function navigate(next: AppSection): void {
    setMoreOpen(false);
    void onNavigate(next);
  }

  if (compact) {
    const primary = ITEMS.filter((item) => COMPACT_SECTIONS.has(item.section));
    return (
      <>
        <nav className={styles.compactNav} aria-label="Main navigation">
          {primary.map((item) => (
            <button
              key={item.section}
              type="button"
              className={styles.compactButton}
              data-active={isActive(item.section, section)}
              aria-current={isActive(item.section, section) ? "page" : undefined}
              onClick={() => navigate(item.section)}
            >
              <span aria-hidden="true">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          ))}
          <button
            type="button"
            className={styles.compactButton}
            data-active={!COMPACT_SECTIONS.has(section) && !isActive("decks", section)}
            aria-expanded={moreOpen}
            onClick={() => setMoreOpen(true)}
          >
            <span aria-hidden="true">•••</span>
            <span>More</span>
          </button>
        </nav>
        {moreOpen ? (
          <Sheet title="More" description="Additional ostudy sections" onClose={() => setMoreOpen(false)}>
            <div className={styles.moreList}>
              {ITEMS.filter((item) => !COMPACT_SECTIONS.has(item.section)).map((item) => (
                <button key={item.section} type="button" onClick={() => navigate(item.section)}>
                  <span aria-hidden="true">{item.icon}</span>
                  <span>{item.label}</span>
                </button>
              ))}
            </div>
          </Sheet>
        ) : null}
      </>
    );
  }

  return (
    <aside className={`sidebar ${styles.sidebar}`}>
      <div className="brand-block">
        <strong>ostudy</strong>
      </div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        {ITEMS.map((item) => (
          <button
            key={item.section}
            type="button"
            className={isActive(item.section, section) ? "nav-button active" : "nav-button"}
            aria-current={isActive(item.section, section) ? "page" : undefined}
            onClick={() => navigate(item.section)}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
