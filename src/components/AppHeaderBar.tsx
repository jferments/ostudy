import type { ReactNode } from "react";

import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { destinationOf, type AppDestination } from "../navigation/appDestinations";
import { IconButton } from "../ui/icons/IconButton";
import { AppHeader } from "../ui/layout/AppFrame";
import { RailAction } from "../ui/layout/AppNavigation";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { BrandLockup, BrandMark } from "./Brand";

interface AppHeaderBarProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  /** The main navigation: in the side rail on wider screens, at the bottom on a phone. */
  navigation: ReactNode;
  onSearch: () => void;
  onSettings: () => void;
}

/** A phone's header names where the learner is; Home shows the brand alone. */
const COMPACT_TITLES: Record<AppDestination, string | null> = {
  home: null,
  library: "Library",
  notes: "Notes",
  tutor: "Tutor",
  progress: "Progress",
};

/**
 * The header on every screen size. Wider screens draw it as a side rail: the
 * raccoon, the main navigation, then Search and Settings at the bottom. A phone
 * has a top bar with the brand, the current place, Search and Settings, since
 * its bottom bar holds the five places.
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
  if (compact) {
    const actionClassName =
      "tw:size-11 tw:rounded-full tw:border-border tw:bg-card tw:text-foreground tw:enabled:hover:bg-muted";
    return (
      <>
        <AppHeader
          data-testid="app-header"
          brand={<BrandLockup compact />}
          title={compactTitle(section, activeWorkflow)}
          actions={
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
                data-testid="app-settings"
                aria-current={settingsActive ? "page" : undefined}
                className={actionClassName}
                onClick={onSettings}
              />
            </>
          }
        />
        {navigation}
      </>
    );
  }
  return (
    <AppHeader
      data-testid="app-header"
      brand={<BrandMark className="tw:size-10" />}
      navigation={navigation}
      actions={
        <>
          <RailAction icon="search" label="Search" data-testid="app-search" onClick={onSearch} />
          <RailAction
            icon="settings"
            label="Settings"
            data-testid="app-settings"
            aria-current={settingsActive ? "page" : undefined}
            onClick={onSettings}
          />
        </>
      }
    />
  );
}

function compactTitle(section: AppSection, activeWorkflow: AppWorkflow | null): string | null {
  if (activeWorkflow === "practice") return "Practice";
  if (section === "settings") return "Settings";
  const destination = destinationOf(section, activeWorkflow);
  return destination === null ? null : COMPACT_TITLES[destination];
}
