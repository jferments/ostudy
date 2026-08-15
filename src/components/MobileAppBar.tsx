import type { AppSection } from "../controllers/useAppController";
import { BrandLockup } from "./Brand";
import styles from "./MobileAppBar.module.css";

const TITLES: Record<AppSection, string> = {
  decks: "Flashcards",
  documents: "Documents",
  cards: "Search",
  templates: "Templates",
  practice: "Practice",
  analytics: "Analytics",
  settings: "Settings",
};

export function MobileAppBar({ section }: { section: AppSection }) {
  return (
    <header className={styles.bar} data-testid="mobile-app-bar">
      <BrandLockup compact />
      <strong className={styles.title}>{TITLES[section]}</strong>
      <span className={styles.spacer} aria-hidden="true" />
    </header>
  );
}
