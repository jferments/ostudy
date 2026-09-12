import type { AppSection, AppWorkflow } from "../controllers/useAppController";
import { type IconName } from "../ui/icons/Icon";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import { BrandLockup } from "./Brand";
import { AppNavigation } from "../ui/layout/AppNavigation";

interface NavigationSidebarProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

const ITEMS: ReadonlyArray<{ section: AppSection; icon: IconName; label: string }> = [
  { section: "decks", icon: "cards", label: "Flashcards" },
  { section: "study-guides", icon: "study-guide", label: "Study Guides" },
  { section: "documents", icon: "document", label: "Documents" },
  { section: "tutor", icon: "brain", label: "Tutor" },
  { section: "analytics", icon: "analytics", label: "Analytics" },
  { section: "settings", icon: "settings", label: "Settings" },
];

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
  return <AppNavigation brand={<BrandLockup compact={layoutMode === "medium"} />} items={ITEMS.map((item) => ({
    id: item.section, label: item.label, icon: item.icon,
    active: isActive(item.section, section, activeWorkflow),
    secondary: item.section === "settings",
    onSelect: () => { void onNavigate(item.section); },
  }))} />;
}
