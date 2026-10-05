import type { ReactNode } from "react";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { destinationOf, type AppDestination } from "../navigation/appDestinations";
import { IconButton } from "../ui/icons/IconButton";
import { AppHeader } from "../ui/layout/AppFrame";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { BrandLockup, BrandMark } from "./Brand";

interface AppHeaderBarProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  /** The main navigation: in the header on wider screens, at the bottom on a phone. */
  navigation: ReactNode;
  onSearch: () => void;
  onSettings: () => void;
}

/** A phone's header names where the learner is; Home shows the brand alone. */
const COMPACT_TITLES: Record<AppDestination, string | null> = {
  home: null,
  library: "Library",
  review: "Review",
  tutor: "Tutor",
  progress: "Progress",
};

/**
 * The header on every screen size: the brand, the main navigation on wider
 * screens, then Search and the Settings gear.
 */
export function AppHeaderBar({
  section,
  activeWorkflow,
  navigation,
  onSearch,
  onSettings,
}: AppHeaderBarProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const settingsActive = activeWorkflow === null && section === "settings";
  const actionClassName =
    "tw:size-11 tw:rounded-full tw:border-border tw:bg-card tw:text-foreground tw:enabled:hover:bg-muted";
  const actions = (
    <>
      <IconButton
        icon="search"
        label="Search"
        tooltip="Search cards"
        data-testid="app-search"
        className={actionClassName}
        onClick={onSearch}
      />
      <IconButton
        icon="settings"
        label="Settings"
        tooltip="Settings"
        data-testid="app-settings"
        aria-current={settingsActive ? "page" : undefined}
        className={actionClassName}
        onClick={onSettings}
      />
    </>
  );
  if (compact) {
    return (
      <>
        <AppHeader
          data-testid="app-header"
          brand={<BrandLockup compact />}
          title={compactTitle(section, activeWorkflow)}
          actions={actions}
        />
        {navigation}
      </>
    );
  }
  return (
    <AppHeader
      data-testid="app-header"
      brand={layoutMode === "medium" ? <BrandMark className="tw:size-9" /> : <BrandLockup />}
      navigation={navigation}
      actions={actions}
    />
  );
}

function compactTitle(section: AppSection, activeWorkflow: AppWorkflow | null): string | null {
  if (activeWorkflow === "practice") return "Practice";
  if (section === "settings") return "Settings";
  const destination = destinationOf(section, activeWorkflow);
  return destination === null ? null : COMPACT_TITLES[destination];
}
