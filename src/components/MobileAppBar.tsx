import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { IconButton } from "../ui/icons/IconButton";
import { BrandLockup } from "./Brand";
import { CompactAppHeader } from "../ui/layout/AppFrame";

const TITLES: Record<AppSection, string> = {
  home: "Home",
  decks: "Flashcards",
  "study-guides": "Study Guides",
  documents: "Documents",
  review: "Review",
  tutor: "Tutor",
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
  const settingsActive = activeWorkflow === null && section === "settings";
  return (
    <CompactAppHeader data-testid="mobile-app-bar">
      <BrandLockup compact />
      {title ? <strong className="tw:truncate tw:text-small tw:font-semibold tw:text-foreground">{title}</strong> : <span aria-hidden="true" />}
      <div className="tw:flex tw:min-w-0 tw:justify-end">
        <IconButton
          icon="settings"
          label="Settings"
          tooltip="Settings"
          data-testid="app-settings"
          aria-current={settingsActive ? "page" : undefined}
          onClick={() => void onNavigate("settings")}
        />
      </div>
    </CompactAppHeader>
  );
}
