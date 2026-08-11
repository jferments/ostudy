import type { TimelineBucket } from "../../ipc/analytics";

export interface DifficultyTimelineSeries {
  reviewed: number[];
  wrong: number[];
  hard: number[];
  medium: number[];
  easy: number[];
}

export function buildDifficultyTimelineSeries(
  buckets: TimelineBucket[],
): DifficultyTimelineSeries {
  return {
    reviewed: buckets.map((bucket) => bucket.reviewCount),
    wrong: buckets.map((bucket) => bucket.wrongCount),
    hard: buckets.map((bucket) => bucket.hardCount),
    medium: buckets.map((bucket) => bucket.mediumCount),
    easy: buckets.map((bucket) => bucket.easyCount),
  };
}
