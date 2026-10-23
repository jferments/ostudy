import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { destinationOf, type AppDestination } from "../navigation/appDestinations";
import type { IconName } from "../ui/icons/Icon";
import { AppNavigation, type NavigationBadge } from "../ui/layout/AppNavigation";

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
  { destination: "notes", icon: "sticky-note", label: "Notes" },
  { destination: "tutor", icon: "chat", label: "Tutor" },
  { destination: "progress", icon: "analytics", label: "Progress" },
];

/**
 * Home, Library, Notes, Tutor and Progress; Home carries today's count.
 * Settings sits at the foot of the side rail, or in a phone's top bar.
 */
export function MainNavigation({ section, activeWorkflow, dueToday, onSelect }: MainNavigationProps) {
  const active = destinationOf(section, activeWorkflow);
  return (
    <AppNavigation
      items={ITEMS.map((item) => ({
        id: item.destination,
        label: item.label,
        icon: item.icon,
        active: item.destination === active,
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
