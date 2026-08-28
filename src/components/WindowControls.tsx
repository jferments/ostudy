import type { WindowControlsController } from "../controllers/useWindowControlsController";
import { IconButton } from "../ui/icons/IconButton";
import { useLayoutMode } from "../ui/layout/LayoutModeProvider";
import styles from "./WindowControls.module.css";

export function WindowControls({
  controller,
  placement,
}: {
  controller: WindowControlsController;
  placement: "sidebar" | "app-bar";
}) {
  const { layoutMode } = useLayoutMode();
  if (
    (placement === "app-bar" && layoutMode !== "compact") ||
    (placement === "sidebar" && layoutMode === "compact")
  ) {
    return null;
  }
  if (!controller.maximizeRestoreAvailable && !controller.fullScreenAvailable) {
    return null;
  }

  const busy = controller.pendingAction !== null;
  const maximizePending =
    controller.pendingAction === "maximize" || controller.pendingAction === "restore";
  const fullScreenPending =
    controller.pendingAction === "enter-full-screen" ||
    controller.pendingAction === "exit-full-screen";
  const maximizeLabel = controller.maximized ? "Restore window" : "Maximize window";
  const fullScreenLabel = controller.fullScreen
    ? "Exit full screen"
    : "Enter full screen";

  return (
    <div className={styles.group} role="group" aria-label="Window controls">
      {controller.maximizeRestoreAvailable ? (
        <IconButton
          icon={controller.maximized ? "restore" : "maximize"}
          label={maximizeLabel}
          tooltip={maximizeLabel}
          aria-pressed={controller.maximized}
          loading={maximizePending}
          disabled={!controller.ready || busy}
          onClick={() => void controller.toggleMaximized()}
        />
      ) : null}
      {controller.fullScreenAvailable ? (
        <IconButton
          icon={controller.fullScreen ? "restore" : "full-screen"}
          label={fullScreenLabel}
          tooltip={fullScreenLabel}
          aria-pressed={controller.fullScreen}
          loading={fullScreenPending}
          disabled={!controller.ready || busy}
          onClick={() => void controller.toggleFullScreen()}
        />
      ) : null}
    </div>
  );
}
