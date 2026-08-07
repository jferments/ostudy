import type { AppSection } from "../controllers/useAppController";

interface NavigationSidebarProps {
  section: AppSection;
  onNavigate: (section: AppSection) => void;
}

const ITEMS: ReadonlyArray<{ section: AppSection; icon: string; label: string }> = [
  { section: "decks", icon: "▤", label: "Decks" },
  { section: "cards", icon: "⌕", label: "Search/Filter" },
  { section: "templates", icon: "▧", label: "Templates" },
  { section: "practice", icon: "▶", label: "Practice" },
  { section: "analytics", icon: "▥", label: "Analytics" },
  { section: "settings", icon: "⚙", label: "Settings" },
];

export function NavigationSidebar({ section, onNavigate }: NavigationSidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand-block">
        <strong>ostudy</strong>
      </div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        {ITEMS.map((item) => (
          <button
            key={item.section}
            type="button"
            className={section === item.section ? "nav-button active" : "nav-button"}
            aria-current={section === item.section ? "page" : undefined}
            onClick={() => onNavigate(item.section)}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
