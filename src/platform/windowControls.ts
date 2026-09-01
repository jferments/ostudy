import { getCurrentWindow } from "@tauri-apps/api/window";

export interface NativeWindowControlState {
  maximized: boolean;
  fullScreen: boolean;
}

export interface WindowControlsAdapter {
  readState(): Promise<NativeWindowControlState>;
  maximize(): Promise<void>;
  restore(): Promise<void>;
  setFullScreen(enabled: boolean): Promise<void>;
  subscribe(listener: () => void): Promise<() => void>;
}

/**
 * Owns direct access to Tauri's main-window API. React controllers consume this
 * adapter and never infer native window state from the browser viewport.
 */
export const nativeWindowControls: WindowControlsAdapter = {
  async readState() {
    const currentWindow = getCurrentWindow();
    const [maximized, fullScreen] = await Promise.all([
      currentWindow.isMaximized(),
      currentWindow.isFullscreen(),
    ]);
    return { maximized, fullScreen };
  },

  maximize() {
    return getCurrentWindow().maximize();
  },

  restore() {
    return getCurrentWindow().unmaximize();
  },

  setFullScreen(enabled) {
    return getCurrentWindow().setFullscreen(enabled);
  },

  subscribe(listener) {
    return getCurrentWindow().onResized(() => listener());
  },
};
