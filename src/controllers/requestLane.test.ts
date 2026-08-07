import { describe, expect, it } from "vitest";

import { deferred } from "../test/factories";
import { latestResult, RequestLane } from "./requestLane";

describe("RequestLane", () => {
  it("accepts only the latest reverse-order completion", async () => {
    const lane = new RequestLane();
    const older = deferred<string>();
    const newer = deferred<string>();

    const olderResult = latestResult(lane, () => older.promise);
    const newerResult = latestResult(lane, () => newer.promise);

    newer.resolve("newer");
    expect(await newerResult).toBe("newer");
    older.resolve("older");
    expect(await olderResult).toBeUndefined();
  });

  it("invalidates an in-flight read without cancelling native work", async () => {
    const lane = new RequestLane();
    const request = deferred<number>();
    const result = latestResult(lane, () => request.promise);
    lane.invalidate();
    request.resolve(42);
    expect(await result).toBeUndefined();
  });

  it("keeps independent lanes independent", async () => {
    const search = new RequestLane();
    const tags = new RequestLane();
    const searchRequest = deferred<string>();
    const tagRequest = deferred<string>();
    const searchResult = latestResult(search, () => searchRequest.promise);
    const tagResult = latestResult(tags, () => tagRequest.promise);

    search.invalidate();
    searchRequest.resolve("obsolete");
    tagRequest.resolve("current");

    expect(await searchResult).toBeUndefined();
    expect(await tagResult).toBe("current");
  });
});
