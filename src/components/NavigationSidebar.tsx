import { useState } from "react";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { Icon, type IconName } from "../ui/icons/Icon";
import { Sheet } from "../ui/overlays/Dialog";
import { BrandLockup } from "./Brand";
import styles from "./NavigationSidebar.module.css";

interface NavigationSidebarProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

const ITEMS: ReadonlyArray<{ section: AppSection; icon: IconName; label: string }> = [
  { section: "decks", icon: "cards", label: "Flashcards" },
  { section: "study-guides", icon: "study-guide", label: "Study Guides" },
  { section: "documents", icon: "document", label: "Documents" },
  { section: "analytics", icon: "analytics", label: "Analytics" },
  { section: "settings", icon: "settings", label: "Settings" },
];

const COMPACT_PRIMARY_SECTIONS = ["decks", "study-guides", "analytics"] as const;
const COMPACT_PRIMARY_SET = new Set<AppSection>(COMPACT_PRIMARY_SECTIONS);
const COMPACT_MORE_SECTIONS = ITEMS.filter(
  (item) => !COMPACT_PRIMARY_SET.has(item.section),
).map((item) => item.section);
const COMPACT_MORE_SET = new Set<AppSection>(COMPACT_MORE_SECTIONS);

function isActive(
  item: AppSection,
  current: AppSection,
  activeWorkflow: AppWorkflow | null,
): boolean {
  if (activeWorkflow) return false;
  if (item === "decks") {
    return current === "decks" || current === "cards" || current === "templates";
  }
  return item === current;
}

export function NavigationSidebar({
  section,
  activeWorkflow,
  onNavigate,
}: NavigationSidebarProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const medium = layoutMode === "medium";
  const [moreOpen, setMoreOpen] = useState(false);

  function navigate(next: AppSection): void {
    setMoreOpen(false);
    void onNavigate(next);
  }

  if (compact) {
    const primary = COMPACT_PRIMARY_SECTIONS.map((primarySection) =>
      ITEMS.find((item) => item.section === primarySection),
    ).filter((item): item is (typeof ITEMS)[number] => item !== undefined);
    const more = COMPACT_MORE_SECTIONS.map((moreSection) =>
      ITEMS.find((item) => item.section === moreSection),
    ).filter((item): item is (typeof ITEMS)[number] => item !== undefined);
    const moreActive = activeWorkflow === null && COMPACT_MORE_SET.has(section);
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
              data-active={isActive(item.section, section, activeWorkflow)}
              aria-current={
                isActive(item.section, section, activeWorkflow) ? "page" : undefined
              }
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
            data-active={moreActive}
            aria-current={moreActive ? "page" : undefined}
            aria-expanded={moreOpen}
            aria-haspopup="dialog"
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
              {more.map((item) => (
                <button
                  key={item.section}
                  type="button"
                  data-testid={`nav-${item.section}`}
                  aria-current={
                    isActive(item.section, section, activeWorkflow) ? "page" : undefined
                  }
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
          const active = isActive(item.section, section, activeWorkflow);
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
