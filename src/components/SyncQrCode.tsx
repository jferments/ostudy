import { useEffect, useRef } from "react";

interface SyncQrCodeProps {
  size: number;
  modulesBase64: string;
  label: string;
}

const QUIET_ZONE_MODULES = 4;
const DISPLAY_PIXELS = 280;

export function SyncQrCode({ size, modulesBase64, label }: SyncQrCodeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size <= 0 || !modulesBase64) return;
    const binary = atob(modulesBase64);
    const packed = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const moduleCount = size + QUIET_ZONE_MODULES * 2;
    const scale = Math.max(1, Math.floor(DISPLAY_PIXELS / moduleCount));
    const pixels = moduleCount * scale;
    const deviceScale = Math.max(1, Math.ceil(window.devicePixelRatio || 1));
    canvas.width = pixels * deviceScale;
    canvas.height = pixels * deviceScale;
    canvas.style.width = `${pixels}px`;
    canvas.style.height = `${pixels}px`;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.imageSmoothingEnabled = false;
    context.scale(deviceScale, deviceScale);
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, pixels, pixels);
    context.fillStyle = "#000000";
    for (let row = 0; row < size; row += 1) {
      for (let column = 0; column < size; column += 1) {
        const index = row * size + column;
        const dark = (packed[Math.floor(index / 8)] & (1 << (7 - (index % 8)))) !== 0;
        if (!dark) continue;
        context.fillRect(
          (column + QUIET_ZONE_MODULES) * scale,
          (row + QUIET_ZONE_MODULES) * scale,
          scale,
          scale,
        );
      }
    }
  }, [modulesBase64, size]);

  return <canvas ref={canvasRef} role="img" aria-label={label} />;
}
