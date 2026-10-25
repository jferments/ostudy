import { useEffect, useState } from "react";

import { getSpeechInputStatus } from "../../ipc/speechInput";
import { microphoneAvailable } from "../../platform/microphone";
import { Panel } from "../../ui/primitives/Panel";
import { SelectControl } from "./SettingsControls";

const PAUSES = [
  { value: "0", label: "When I press stop" },
  { value: "1", label: "After 1 second of silence" },
  { value: "2", label: "After 2 seconds of silence" },
  { value: "3", label: "After 3 seconds of silence" },
  { value: "5", label: "After 5 seconds of silence" },
] as const;

type Pause = (typeof PAUSES)[number]["value"];

/**
 * When a question spoken to the Tutor is sent: on a pause, or only on stop.
 * Hidden where this build has no speech input.
 */
export function SpeechInputSettings({ seconds, onChange }: { seconds: number; onChange: (seconds: number) => void }) {
  const [supported, setSupported] = useState(false);
  useEffect(() => {
    let current = true;
    void getSpeechInputStatus().then(
      (status) => { if (current) setSupported(status.supported && microphoneAvailable()); },
      () => undefined,
    );
    return () => { current = false; };
  }, []);
  if (!supported) return null;
  const value = PAUSES.find((pause) => Number(pause.value) === seconds)?.value ?? "2";
  return (
    <Panel className="tw:min-w-0" title="Speech input">
      <SelectControl<Pause>
        label="Send a spoken question"
        value={value}
        options={PAUSES}
        onChange={(pause) => onChange(Number(pause))}
      />
    </Panel>
  );
}
