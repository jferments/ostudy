import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { loadVoice } from "./voiceReady";

/**
 * Whether a lesson voice is set up, so Listen can stay hidden without one.
 * Unknown (and hidden) until the setting loads, and hidden when it cannot load.
 */
export function useVoiceAvailable(): boolean {
  const lane = useRequestLane();
  const [available, setAvailable] = useState(false);

  useEffect(() => {
    void latestResult(lane, loadVoice).then(
      (voice) => {
        if (voice !== undefined) setAvailable(voice.ready);
      },
      () => setAvailable(false),
    );
  }, [lane]);

  return available;
}
