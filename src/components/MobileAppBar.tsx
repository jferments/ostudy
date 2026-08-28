import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { BrandLockup } from "./Brand";
import styles from "./MobileAppBar.module.css";

const TITLES: Record<AppSection, string> = {
  decks: "Flashcards",
  "study-guides": "Study Guides",
  documents: "Documents",
  cards: "Search",
  templates: "Templates",
  analytics: "Analytics",
  settings: "Settings",
};

export function MobileAppBar({
  section,
  activeWorkflow,
}: {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
}) {
  const title = activeWorkflow === "practice" ? "Practice" : section === "decks" ? null : TITLES[section];
  return (
    <header className={styles.bar} data-testid="mobile-app-bar">
      <BrandLockup compact />
      {title ? <strong className={styles.title}>{title}</strong> : <span aria-hidden="true" />}
      <span className={styles.spacer} aria-hidden="true" />
    </header>
  );
}
