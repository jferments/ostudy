import { useCallback, useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getMaterialInk, type InkMark, type InkMaterial } from "../../ipc/materialInk";
import { inkMaterialKey } from "./materialInkSource";
import { marksByPage } from "./InkOverlay";

const NO_MARKS: ReadonlyMap<string, readonly InkMark[]> = new Map();

/**
 * The learner's marks on a material by page key, to draw over its designed
 * pages. `reload` reads them again, as after the ink editor closes. A failed
 * read shows the pages without marks.
 */
export function useMaterialInk(material: InkMaterial | null): {
  marks: ReadonlyMap<string, readonly InkMark[]>;
  reload: () => void;
} {
  const lane = useRequestLane();
  const [marks, setMarks] = useState<{ key: string; pages: ReadonlyMap<string, readonly InkMark[]> } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const key = material === null ? null : inkMaterialKey(material);

  useEffect(() => {
    if (material === null || key === null) return;
    let live = true;
    void latestResult(lane, () => getMaterialInk(material)).then(
      (value) => {
        if (value !== undefined && live) setMarks({ key, pages: marksByPage(value) });
      },
      () => {
        if (live) setMarks({ key, pages: NO_MARKS });
      },
    );
    return () => {
      live = false;
    };
    // The material is named by its key; a new object for the same material reads nothing again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lane, key, attempt]);

  const reload = useCallback(() => setAttempt((value) => value + 1), []);
  return { marks: marks !== null && marks.key === key ? marks.pages : NO_MARKS, reload };
}
