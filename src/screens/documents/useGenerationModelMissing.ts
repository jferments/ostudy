import { useEffect, useState } from "react";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";

/**
 * True once the read shows no service chosen for cards and guides. Unknown
 * (loading, or a read that failed) counts as set, so the native side still
 * reports its own refusal. Settings is another destination, so returning from
 * it remounts the page and reads again.
 */
export function useGenerationModelMissing(): boolean {
  const [missing, setMissing] = useState(false);
  useEffect(() => {
    let active = true;
    void Promise.all([getAiTaskDefaults(), listAiProviderProfiles()])
      .then(([defaults, profiles]) => {
        const profileId = defaults.flashcardGenerationProfileId;
        if (active) setMissing(!profileId || !profiles.some((profile) => profile.id === profileId));
      })
      .catch(() => { if (active) setMissing(false); });
    return () => { active = false; };
  }, []);
  return missing;
}
