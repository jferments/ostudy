import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Tooltip } from "../primitives/Tooltip";
import { Button } from "../primitives/Button";
import { Icon, type IconName } from "./Icon";
export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-busy" | "aria-label" | "children" | "title"> {
  icon: IconName; label: string; tooltip?: string | false; tone?: "neutral" | "danger"; loading?: boolean; iconSize?: 16 | 20 | 24 | 32;
}
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton({ icon, label, tooltip, tone = "neutral", loading = false, disabled = false, iconSize = 20, className, type = "button", ...props }, ref) {
  const name = label.trim();
  if (!name) throw new Error("IconButton requires a nonblank accessible label.");
  const hint = tooltip === false ? null : tooltip?.trim() || name;
  const button = <Button {...props} ref={ref} type={type} size="icon" variant={tone === "danger" ? "dangerGhost" : "ghost"} className={className} data-tone={tone} aria-label={name} loading={loading} disabled={disabled}>
    <Icon name={icon} size={iconSize} />
  </Button>;
  return hint ? <Tooltip content={hint}>{button}</Tooltip> : button;
});
