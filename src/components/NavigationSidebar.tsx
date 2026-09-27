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

const ITEMS: ReadonlyArray<{ section: AppSection; icon: IconName; label: string; compactLabel?: string }> = [
  { section: "decks", icon: "cards", label: "Flashcards" },
  { section: "study-guides", icon: "study-guide", label: "Study Guides", compactLabel: "Study\nGuides" },
  { section: "documents", icon: "document", label: "Documents", compactLabel: "Docs" },
  { section: "tutor", icon: "brain", label: "Tutor" },
  { section: "analytics", icon: "analytics", label: "Analytics" },
  { section: "settings", icon: "settings", label: "Settings" },
];

const COMPACT_PRIMARY_SECTIONS = [
  "decks",
  "study-guides",
  "documents",
  "tutor",
  "analytics",
] as const;

function isActive(
  item: AppSection,
  current: AppSection,
  activeWorkflow: AppWorkflow | null,
): boolean {
  return activeWorkflow === null && item === current;
}

export function NavigationSidebar({
  section,
  activeWorkflow,
  onNavigate,
}: NavigationSidebarProps) {
  const { layoutMode } = useLayoutMode();
  return <AppNavigation brand={<BrandLockup compact={layoutMode === "medium"} />} items={ITEMS.map((item) => ({
    id: item.section, label: item.label, compactLabel: item.compactLabel, icon: item.icon,
    active: isActive(item.section, section, activeWorkflow),
    secondary: !COMPACT_PRIMARY_SECTIONS.some((section) => section === item.section),
    onSelect: () => { void onNavigate(item.section); },
  }))} />;
}
