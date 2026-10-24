import { useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { getErrorMessage } from "../../ipc/common";
import { deleteSubject, getSubjectLibrary, moveSubject, renameSubject, type SubjectLibrary } from "../../ipc/subjects";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { Icon } from "../../ui/icons/Icon";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { buildSubjectTiles, countLabel, type SubjectTile } from "../home/homeModel";
import { SubjectBadge } from "../home/SubjectBadge";
import { MAX_SESSION_DECKS } from "../home/TodayStrip";
import { NewStudySetSheet } from "./NewStudySetSheet";
import { NameSheet } from "./workbench/WorkbenchSheets";

const NO_PREPARATIONS = new Map();
const MAX_SUBJECT_NAME = 120;

export interface LibrarySubjects {
  subjects: readonly SubjectTile[];
  noSubject: SubjectTile | null;
  /** The chosen subject's tile; null for Everything, or while it loads. */
  current: SubjectTile | null;
  /** True once the subject list has loaded. */
  ready: boolean;
}

/** Library's subjects, in the learner's order, from the summary Home reads too. */
export function useLibrarySubjects(scope: LibrarySubjectScope): LibrarySubjects {
  const home = useOptionalHomeSummary();
  const summary = home?.summary ?? null;
  const library = home?.library ?? null;
  return useMemo(() => {
    if (!summary || !library) return { subjects: [], noSubject: null, current: null, ready: false };
    const { subjects, noSubject } = buildSubjectTiles(summary, library, NO_PREPARATIONS);
    const current =
      scope.kind === "subject"
        ? (subjects.find((tile) => tile.subjectId === scope.subjectId) ?? null)
        : scope.kind === "none"
          ? noSubject
          : null;
    return { subjects, noSubject, current, ready: true };
  }, [library, scope, summary]);
}

interface LibrarySubjectHeaderProps {
  scope: LibrarySubjectScope;
  subjects: LibrarySubjects;
  onChooseSubject: (subject: LibrarySubjectScope) => void;
  onStartReview: (deckIds: string[]) => void;
  /** One extra session mixing cards from every deck in the subject. */
  onPracticeSubject: (subjectId: string) => void;
  /** Shows the subject's Documents, where materials added from its menu are read and made. */
  onOpenDocuments: () => void;
  showToast: ShowToast;
}

/**
 * The chosen subject's name, what it holds, its Review button and its own
 * actions (add materials, rename, reorder, delete); All cards totals every subject.
 */
export function LibrarySubjectHeader({
  scope,
  subjects,
  onChooseSubject,
  onStartReview,
  onPracticeSubject,
  onOpenDocuments,
  showToast,
}: LibrarySubjectHeaderProps) {
  const home = useOptionalHomeSummary();
  const [adding, setAdding] = useState(false);
  const all = [...subjects.subjects, ...(subjects.noSubject ? [subjects.noSubject] : [])];
  const shown = scope.kind === "all" ? all : subjects.current ? [subjects.current] : [];
  const sources = shown.reduce((sum, tile) => sum + tile.summary.sourceCount, 0);
  const cards = shown.reduce((sum, tile) => sum + tile.summary.cardCount, 0);
  const due = shown.reduce((sum, tile) => sum + tile.dueCount, 0);
  const dueDeckIds = [...new Set(shown.flatMap((tile) => tile.summary.dueDeckIds))];
  const name = scope.kind === "all" ? "All cards" : scope.kind === "none" ? "No subject" : (subjects.current?.name ?? "");
  const counts = [
    ...(scope.kind === "all" ? [countLabel(subjects.subjects.length, "subject")] : []),
    countLabel(sources, "document"),
    countLabel(cards, "card"),
  ].join(" · ");

  // Each page under the tabs names itself in its h1; the subject is the page's context.
  const title = (
    <h2
      data-testid="library-subject-name"
      className="tw:m-0 tw:min-w-0 tw:truncate tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:compact:text-section"
    >
      {name}
    </h2>
  );
  return (
    <header data-testid="library-subject-header" className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3">
      {subjects.current ? <SubjectBadge subject={subjects.current} size="large" className="tw:compact:size-11" /> : null}
      <div className="tw:min-w-0 tw:flex-[1_1_12rem]">
        {title}
        {subjects.ready ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{counts}</p> : null}
      </div>
      {scope.kind === "subject" && cards > 0 ? (
        <Button data-testid="library-practice" onClick={() => onPracticeSubject(scope.subjectId)}>
          Practice a mix
        </Button>
      ) : null}
      {due > 0 ? (
        <Button
          variant="primary"
          data-testid="library-review"
          disabled={dueDeckIds.length > MAX_SESSION_DECKS}
          onClick={() => onStartReview(dueDeckIds)}
        >
          <Icon name="practice" size={15} /> Review {due.toLocaleString()} due
        </Button>
      ) : null}
      {scope.kind === "subject" && subjects.current ? (
        <SubjectActions
          subjectId={scope.subjectId}
          name={subjects.current.name}
          subjects={subjects}
          onChooseSubject={onChooseSubject}
          onAddMaterials={home?.library ? () => setAdding(true) : undefined}
          showToast={showToast}
        />
      ) : null}
      {adding && scope.kind === "subject" && home?.library ? (
        <NewStudySetSheet
          subjects={home.library.subjects}
          subjectId={scope.subjectId}
          onClose={() => {
            setAdding(false);
            home.refresh();
          }}
          onCreated={() => {
            setAdding(false);
            home.refresh();
            onOpenDocuments();
          }}
        />
      ) : null}
    </header>
  );
}

/** Rename, Move up, Move down and Delete for a subject. */
export function SubjectActions({
  subjectId,
  name,
  subjects,
  onChooseSubject,
  onAddMaterials,
  showToast,
}: {
  subjectId: string;
  name: string;
  subjects: LibrarySubjects;
  onChooseSubject: (subject: LibrarySubjectScope) => void;
  /** Opens New study set on this subject, to add documents and materials to it. */
  onAddMaterials?: () => void;
  showToast: ShowToast;
}) {
  const home = useOptionalHomeSummary();
  const confirm = useConfirm();
  const [renaming, setRenaming] = useState(false);
  const index = subjects.subjects.findIndex((tile) => tile.subjectId === subjectId);

  /** Runs one subject write against the current revisions and shares its answer with the rest of the app. */
  async function write(run: (current: SubjectLibrary) => Promise<SubjectLibrary>): Promise<boolean> {
    try {
      const next = await run(await getSubjectLibrary());
      home?.acceptLibrary(next);
      return true;
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
      home?.refresh();
      return false;
    }
  }
  const revisionOf = (current: SubjectLibrary) => current.subjects.find((subject) => subject.id === subjectId)?.revision ?? 0;

  async function remove(): Promise<void> {
    const accepted = await confirm({
      title: `Delete “${name}”?`,
      description: "Its decks and documents move to No subject, with their cards and schedules. The subject's lessons and overview guide are deleted.",
      confirmLabel: "Delete subject",
      destructive: true,
    });
    if (!accepted) return;
    const deleted = await write((current) =>
      deleteSubject({ subjectId, revision: revisionOf(current), expectedLibraryRevision: current.libraryRevision }),
    );
    if (!deleted) return;
    showToast("success", `Deleted “${name}”.`);
    onChooseSubject({ kind: "all" });
  }

  return (
    <>
      <OverflowMenu
        label={`${name} actions`}
        items={[
          ...(onAddMaterials ? [{ id: "add", label: "Add materials…", onSelect: onAddMaterials }] : []),
          { id: "rename", label: "Rename…", onSelect: () => setRenaming(true) },
          {
            id: "up",
            label: "Move up",
            disabled: index <= 0,
            onSelect: () =>
              void write((current) => moveSubject({ subjectId, targetIndex: index - 1, expectedLibraryRevision: current.libraryRevision })),
          },
          {
            id: "down",
            label: "Move down",
            disabled: index < 0 || index >= subjects.subjects.length - 1,
            onSelect: () =>
              void write((current) => moveSubject({ subjectId, targetIndex: index + 1, expectedLibraryRevision: current.libraryRevision })),
          },
          { id: "delete", label: "Delete subject…", destructive: true, onSelect: () => void remove() },
        ]}
      />
      {renaming ? (
        <NameSheet
          title="Rename subject"
          label="Name"
          initialName={name}
          submitLabel="Rename"
          maxLength={MAX_SUBJECT_NAME}
          onClose={() => setRenaming(false)}
          onSubmit={(next) =>
            write((current) =>
              renameSubject({ subjectId, name: next, revision: revisionOf(current), expectedLibraryRevision: current.libraryRevision }),
            )
          }
        />
      ) : null}
    </>
  );
}
