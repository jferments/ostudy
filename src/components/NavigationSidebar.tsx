import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { Icon, type IconName } from "../ui/icons/Icon";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
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

const COMPACT_SECTIONS = [
  "decks",
  "study-guides",
  "documents",
  "analytics",
] as const;

function isActive(
  item: AppSection,
  current: AppSection,
  activeWorkflow: AppWorkflow | null,
): boolean {
  if (activeWorkflow) return false;
  if (item === "decks") return current === "decks" || current === "cards";
  if (item === "settings") return current === "settings" || current === "templates";
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

  function navigate(next: AppSection): void {
    void onNavigate(next);
  }

  if (compact) {
    const compactItems = COMPACT_SECTIONS.map((compactSection) =>
      ITEMS.find((item) => item.section === compactSection),
    ).filter((item): item is (typeof ITEMS)[number] => item !== undefined);
    return (
      <nav
        className={styles.compactNav}
        aria-label="Main navigation"
        data-tour-anchor="app.navigation"
      >
        {compactItems.map((item) => {
          const active = isActive(item.section, section, activeWorkflow);
          return (
            <button
              key={item.section}
              type="button"
              className={styles.compactButton}
              data-testid={`nav-${item.section}`}
              data-active={active}
              aria-current={active ? "page" : undefined}
              onClick={() => navigate(item.section)}
            >
              <Icon name={item.icon} size={20} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
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
