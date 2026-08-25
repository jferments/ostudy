import { useState } from "react";

import type { AppSection } from "../controllers/useAppController";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { Icon, type IconName } from "../ui/icons/Icon";
import { Sheet } from "../ui/overlays/Dialog";
import { BrandLockup } from "./Brand";
import styles from "./NavigationSidebar.module.css";

interface NavigationSidebarProps {
  section: AppSection;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

const ITEMS: ReadonlyArray<{ section: AppSection; icon: IconName; label: string }> = [
  { section: "decks", icon: "cards", label: "Flashcards" },
  { section: "documents", icon: "document", label: "Documents" },
  { section: "practice", icon: "practice", label: "Practice" },
  { section: "analytics", icon: "analytics", label: "Analytics" },
  { section: "settings", icon: "settings", label: "Settings" },
];

const COMPACT_SECTIONS = new Set<AppSection>(["decks", "practice", "analytics"]);

function isActive(item: AppSection, current: AppSection): boolean {
  if (item === "decks") {
    return current === "decks" || current === "cards" || current === "templates";
  }
  return item === current;
}

export function NavigationSidebar({ section, onNavigate }: NavigationSidebarProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const medium = layoutMode === "medium";
  const [moreOpen, setMoreOpen] = useState(false);

  function navigate(next: AppSection): void {
    setMoreOpen(false);
    void onNavigate(next);
  }

  if (compact) {
    const primary = ITEMS.filter((item) => COMPACT_SECTIONS.has(item.section));
    return (
      <>
        <nav
          className={styles.compactNav}
          aria-label="Main navigation"
          data-tour-anchor="app.navigation"
        >
          {primary.map((item) => (
            <button
              key={item.section}
              type="button"
              className={styles.compactButton}
              data-testid={`nav-${item.section}`}
              data-active={isActive(item.section, section)}
              aria-current={isActive(item.section, section) ? "page" : undefined}
              onClick={() => navigate(item.section)}
            >
              <Icon name={item.icon} size={20} />
              <span>{item.label}</span>
            </button>
          ))}
          <button
            type="button"
            className={styles.compactButton}
            data-testid="nav-more"
            data-active={!COMPACT_SECTIONS.has(section) && !isActive("decks", section)}
            aria-expanded={moreOpen}
            onClick={() => setMoreOpen(true)}
          >
            <Icon name="menu" size={20} />
            <span>More</span>
          </button>
        </nav>
        {moreOpen ? (
          <Sheet
            title="More"
            description="Additional ostudy sections"
            onClose={() => setMoreOpen(false)}
          >
            <div className={styles.moreList}>
              {ITEMS.filter((item) => !COMPACT_SECTIONS.has(item.section)).map((item) => (
                <button
                  key={item.section}
                  type="button"
                  data-testid={`nav-${item.section}`}
                  onClick={() => navigate(item.section)}
                >
                  <Icon name={item.icon} size={22} />
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
    <aside className={styles.sidebar}>
      <BrandLockup compact={medium} />
      <nav
        className={styles.sidebarNav}
        aria-label="Main navigation"
        data-tour-anchor="app.navigation"
      >
        {ITEMS.map((item) => {
          const active = isActive(item.section, section);
          return (
            <button
              key={item.section}
              type="button"
              data-testid={`nav-${item.section}`}
              className={`${styles.navButton} ${active ? styles.active : ""}`.trim()}
              aria-current={active ? "page" : undefined}
              onClick={() => navigate(item.section)}
            >
              <span className={styles.navIcon}>
                <Icon name={item.icon} size={21} />
              </span>
              <span className={styles.navLabel}>{item.label}</span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
