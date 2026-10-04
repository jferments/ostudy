import { describe, expect, it } from "vitest";

import type { CommandErrorPayload } from "../common";
import type { DeckSummary } from "../decks";
import type { LibrarySession } from "../librarySession";
import type { AppSettings } from "../settings";
import type { SubjectFilingReview, SubjectLibrary } from "../subjects";
import fixtures from "./nativeWireFixtures.json";
import type {
  NativeAppSettings,
  NativeCommandError,
  NativeDeckSummary,
  NativeLibrarySession,
  NativeSubjectFilingReview,
  NativeSubjectLibrary,
  NativeWireEncodingFixture,
} from "./nativeWireContracts";

const asFrontendDeck = (value: NativeDeckSummary): DeckSummary => value;
const asFrontendSettings = (value: NativeAppSettings): AppSettings => value;
const asFrontendError = (value: NativeCommandError): CommandErrorPayload => value;
const asFrontendSession = (value: NativeLibrarySession): LibrarySession => value;
const asFrontendSubjectLibrary = (value: NativeSubjectLibrary): SubjectLibrary => value;
const asFrontendFilingReview = (value: NativeSubjectFilingReview): SubjectFilingReview => value;

describe("native-owned IPC wire fixtures", () => {
  it("keeps generated native product DTOs assignable to frontend contracts", () => {
    expect(asFrontendDeck(fixtures.deckSummary)).toMatchObject({ name: "Physiology" });
    const nativeSettings: NativeAppSettings = {
      ...fixtures.appSettings,
      theme: "dark",
      motionPreference: "system",
      materialPreference: "automatic",
    };
    expect(asFrontendSettings(nativeSettings)).toMatchObject({ theme: "dark" });
    expect(asFrontendError(fixtures.commandError)).toEqual({
      code: "example_error",
      message: "Example safe message.",
    });
    expect(asFrontendSession(fixtures.librarySession).lastRestore).toEqual(fixtures.completedRestore);
    const library = asFrontendSubjectLibrary(fixtures.subjectLibrary);
    expect(library.subjects[0]).toMatchObject({ name: "Pharmacology" });
    // A deck or document without a subject is filed under No subject.
    expect(library.decks[1]?.subjectId).toBeNull();
    const review: NativeSubjectFilingReview = {
      ...fixtures.subjectFilingReview,
      entityKind: "deck",
    };
    expect(asFrontendFilingReview(review).detail).toEqual({
      folderSubjectId: null,
      subjectIds: library.subjects.map((subject) => subject.id),
    });
  });

  it("documents serde byte, nullable number and tagged-union lowering", () => {
    const ready: NativeWireEncodingFixture = {
      kind: "ready",
      bytes: fixtures.encoding.bytes,
      optional: fixtures.encoding.optional,
      signed: fixtures.encoding.signed,
      unsigned: fixtures.encoding.unsigned,
    };
    expect(ready).toEqual({
      kind: "ready",
      bytes: [0, 127, 255],
      optional: null,
      signed: -7,
      unsigned: 42,
    });
    expect(fixtures.deckSummary.archivedAt).toBeNull();
    expect(fixtures.emptyEncoding).toEqual({ kind: "empty" });
  });
});
