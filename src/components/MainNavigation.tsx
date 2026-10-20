import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { destinationOf, type AppDestination } from "../navigation/appDestinations";
import type { IconName } from "../ui/icons/Icon";
import { AppNavigation, type NavigationBadge } from "../ui/layout/AppNavigation";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";

interface MainNavigationProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  /** The cards Start review studies today, or null while that is unknown. */
  dueToday: number | null;
  onSelect: (destination: AppDestination) => void;
}

const ITEMS: ReadonlyArray<{ destination: AppDestination; icon: IconName; label: string }> = [
  { destination: "home", icon: "home", label: "Home" },
  { destination: "library", icon: "library", label: "Library" },
  { destination: "tutor", icon: "chat", label: "Tutor" },
  { destination: "progress", icon: "analytics", label: "Progress" },
  { destination: "settings", icon: "settings", label: "Settings" },
];

/** Home, Library, Tutor, Progress and Settings; Home carries today's count. */
export function MainNavigation({ section, activeWorkflow, dueToday, onSelect }: MainNavigationProps) {
  const { layoutMode } = useLayoutMode();
  const active = destinationOf(section, activeWorkflow);
  // A phone's bottom bar ends with Settings; the side rail keeps it at its foot.
  const items = layoutMode === "compact" ? ITEMS : ITEMS.filter((item) => item.destination !== "settings");
  return (
    <AppNavigation
      items={items.map((item) => ({
        id: item.destination,
        label: item.label,
        icon: item.icon,
        active: item.destination === active,
        testId: item.destination === "settings" ? "app-settings" : undefined,
        badge:
          item.destination === "home" && dueToday !== null && dueToday > 0
            ? dueTodayBadge(dueToday)
            : undefined,
        onSelect: () => onSelect(item.destination),
      }))}
    />
  );
}

export function dueTodayBadge(count: number): NavigationBadge {
  return {
    value: count > 999 ? "999+" : String(count),
    description: `${count.toLocaleString()} ${count === 1 ? "card" : "cards"} due today`,
  };
}
