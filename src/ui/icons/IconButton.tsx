import type { ButtonHTMLAttributes } from "react";

import { Button } from "../primitives/Button";
import { Icon, type IconName } from "./Icon";

export function IconButton({
  icon,
  label,
  size = 20,
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "aria-label"> & {
  icon: IconName;
  label: string;
  size?: 16 | 20 | 24 | 32;
}) {
  return (
    <Button {...props} size="icon" aria-label={label} title={props.title ?? label}>
      <Icon name={icon} size={size} />
    </Button>
  );
}
