import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getSpeechSettings } from "../../ipc/speech";

/**
 * Whether a lesson voice is set up, so Listen can stay hidden without one.
 * Unknown (and hidden) until the setting loads, and hidden when it cannot load.
 */
export function useVoiceAvailable(): boolean {
  const lane = useRequestLane();
  const [available, setAvailable] = useState(false);

  useEffect(() => {
    void latestResult(lane, getSpeechSettings).then(
      (settings) => {
        if (settings !== undefined) setAvailable(settings.profileId !== null);
      },
      () => setAvailable(false),
    );
  }, [lane]);

  return available;
}
