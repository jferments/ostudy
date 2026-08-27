import { OverflowMenu, type OverflowMenuItem } from "./OverflowMenu";

export interface MenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  destructive?: boolean;
}

interface MenuProps {
  label: string;
  items: MenuItem[];
  disabled?: boolean;
  className?: string;
}

export function Menu({ label, items, disabled = false, className = "" }: MenuProps) {
  const overflowItems: OverflowMenuItem[] = items.map((item, index) => ({
    ...item,
    id: `${index}-${item.label}`,
  }));
  return (
    <OverflowMenu
      label={label}
      items={overflowItems}
      disabled={disabled}
      className={className}
      triggerPresentation="text"
    />
  );
}
