import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { IconButton } from "../ui/icons/IconButton";
import { BrandLockup } from "./Brand";
import styles from "./MobileAppBar.module.css";

const TITLES: Record<AppSection, string> = {
  decks: "Flashcards",
  "study-guides": "Study Guides",
  documents: "Documents",
  cards: "Search",
  templates: "Card templates",
  analytics: "Analytics",
  settings: "Settings",
};

interface MobileAppBarProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

export function MobileAppBar({
  section,
  activeWorkflow,
  onNavigate,
}: MobileAppBarProps) {
  const title =
    activeWorkflow === "practice"
      ? "Practice"
      : section === "decks"
        ? null
        : TITLES[section];
  const settingsActive =
    activeWorkflow === null && (section === "settings" || section === "templates");
  return (
    <header className={styles.bar} data-testid="mobile-app-bar">
      <BrandLockup compact />
      {title ? <strong className={styles.title}>{title}</strong> : <span aria-hidden="true" />}
      <div className={styles.actions}>
        <IconButton
          icon="settings"
          label="Settings"
          tooltip="Settings"
          data-testid="app-settings"
          aria-current={settingsActive ? "page" : undefined}
          onClick={() => void onNavigate("settings")}
        />
      </div>
    </header>
  );
}
