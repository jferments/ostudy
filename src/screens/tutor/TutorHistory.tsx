import { useId, useState } from "react";

import { sameStudyScope, scopeMembers, type ScopeChoice, type StudyScope } from "../../ipc/knowledge";
import type { ConversationSummary, TutorProject } from "../../ipc/tutor";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Disclosure, Textarea } from "../../ui/primitives/FormControls";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";
import { TutorSources } from "./TutorSources";
import { styles } from "./presentation";
import type { TutorController, TutorProjectValues } from "./useTutorController";

const GENERAL: StudyScope = { kind: "general", id: null };

function scopeLabel(scope: StudyScope, choices: ScopeChoice[]): string {
  if (scope.kind === "general") return "General discussion";
  if (scope.kind === "library") return "Library";
  const members = scopeMembers(scope);
  if (members.length > 1) return `${members.length} sources`;
  if (members.length === 1) {
    const member = members[0];
    return member
      ? choices.find((choice) => sameStudyScope(choice.scope, member))?.title ?? "Selected source"
      : "Selected source";
  }
  return "General discussion";
}

function ProjectEditor({
  tutor,
  project,
  onClose,
}: {
  tutor: TutorController;
  project: TutorProject | null;
  onClose: () => void;
}) {
  const id = useId();
  const [title, setTitle] = useState(project?.title ?? "");
  const [description, setDescription] = useState(project?.description ?? "");
  const [instructions, setInstructions] = useState(project?.instructions ?? "");
  const [defaultScope, setDefaultScope] = useState<StudyScope>(project?.defaultScope ?? GENERAL);
  const [sourcesOpen, setSourcesOpen] = useState(false);

  async function save(): Promise<void> {
    const values: TutorProjectValues = { title, description, instructions, defaultScope };
    const saved = project
      ? await tutor.updateProject(project, values)
      : await tutor.createProject(values);
    if (saved) onClose();
  }

  return (
    <Dialog
      title={project ? "Edit Tutor project" : "New Tutor project"}
      description="Projects organize chats and provide defaults. They do not merge chat history or grant cross-chat retrieval."
      onClose={onClose}
      busy={tutor.organizing}
      width="medium"
      actions={<>
        <Button disabled={tutor.organizing} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={tutor.organizing || !title.trim()} onClick={() => { void save(); }}>
          {project ? "Save project" : "Create project"}
        </Button>
      </>}
    >
      <label className={styles.field} htmlFor={`${id}-project-title`}>
        Project name
        <TextField
          id={`${id}-project-title`}
          maxLength={160}
          value={title}
          disabled={tutor.organizing}
          onChange={(event) => setTitle(event.currentTarget.value)}
        />
      </label>
      <label className={styles.field} htmlFor={`${id}-project-description`}>
        Description
        <Textarea
          id={`${id}-project-description`}
          rows={3}
          maxLength={4000}
          value={description}
          disabled={tutor.organizing}
          onChange={(event) => setDescription(event.currentTarget.value)}
        />
      </label>
      <label className={styles.field} htmlFor={`${id}-project-instructions`}>
        Tutor instructions
        <Textarea
          id={`${id}-project-instructions`}
          rows={4}
          maxLength={8000}
          value={instructions}
          disabled={tutor.organizing}
          placeholder="Optional preferences for explanation style, emphasis, or study approach."
          onChange={(event) => setInstructions(event.currentTarget.value)}
        />
      </label>
      <div className={styles.projectDefault}>
        <div>
          <strong>Default sources</strong>
          <span className={styles.muted}>{scopeLabel(defaultScope, tutor.choices)}</span>
        </div>
        <Button disabled={tutor.organizing} onClick={() => setSourcesOpen(true)}>Choose sources</Button>
      </div>
      <p className={styles.muted}>New chats inherit the current defaults. Existing chats and historical Tutor turns keep the sources they already captured.</p>
      {sourcesOpen ? (
        <TutorSources
          title="Default project sources"
          description="Choose the sources new chats in this project start with. Existing chats are unchanged."
          note="Selecting defaults is local organization only and sends nothing to an AI provider."
          applyLabel="Use as defaults"
          scope={defaultScope}
          choices={tutor.choices}
          onClose={() => setSourcesOpen(false)}
          onApply={setDefaultScope}
        />
      ) : null}
    </Dialog>
  );
}

function RenameConversationDialog({
  tutor,
  conversation,
  onClose,
}: {
  tutor: TutorController;
  conversation: ConversationSummary;
  onClose: () => void;
}) {
  const id = useId();
  const [title, setTitle] = useState(conversation.title);

  async function save(): Promise<void> {
    if (await tutor.renameConversation(conversation, title)) onClose();
  }

  return (
    <Dialog title="Rename conversation" onClose={onClose} busy={tutor.organizing} width="small"
      actions={<>
        <Button disabled={tutor.organizing} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={tutor.organizing || !title.trim()} onClick={() => { void save(); }}>Rename</Button>
      </>}>
      <label className={styles.field} htmlFor={`${id}-conversation-title`}>
        Conversation name
        <TextField
          id={`${id}-conversation-title`}
          maxLength={200}
          value={title}
          disabled={tutor.organizing}
          onChange={(event) => setTitle(event.currentTarget.value)}
        />
      </label>
    </Dialog>
  );
}

function MoveConversationDialog({
  tutor,
  conversation,
  onClose,
}: {
  tutor: TutorController;
  conversation: ConversationSummary;
  onClose: () => void;
}) {
  const id = useId();
  const activeProjects = tutor.projects.filter((project) => project.archivedAt === null);
  const [destination, setDestination] = useState(conversation.projectId ?? "");
  const [applyDefaults, setApplyDefaults] = useState(false);
  const unchanged = (destination || null) === conversation.projectId && !applyDefaults;

  async function move(): Promise<void> {
    if (await tutor.moveConversation(conversation, destination || null, applyDefaults)) onClose();
  }

  return (
    <Dialog
      title="Move conversation"
      description="Moving changes organization only unless you explicitly apply the destination project's source defaults."
      onClose={onClose}
      busy={tutor.organizing}
      width="small"
      actions={<>
        <Button disabled={tutor.organizing} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={tutor.organizing || unchanged} onClick={() => { void move(); }}>Move conversation</Button>
      </>}
    >
      <label className={styles.field} htmlFor={`${id}-project`}>
        Destination
        <NativeSelect
          id={`${id}-project`}
          aria-label="Destination project"
          value={destination}
          disabled={tutor.organizing}
          onChange={(event) => {
            setDestination(event.currentTarget.value);
            if (!event.currentTarget.value) setApplyDefaults(false);
          }}
        >
          <option value="">Ungrouped</option>
          {activeProjects.map((project) => <option key={project.id} value={project.id}>{project.title}</option>)}
        </NativeSelect>
      </label>
      <label className={styles.checkbox}>
        <Checkbox
          checked={applyDefaults}
          disabled={tutor.organizing || !destination}
          onCheckedChange={setApplyDefaults}
        />
        Use destination project's current default sources
      </label>
      <p className={styles.muted}>When this is off, the conversation keeps its current sources exactly.</p>
    </Dialog>
  );
}

function ConversationRow({
  tutor,
  conversation,
  projectTitle,
  switchLocked,
  organizationLocked,
  onSelected,
  onRename,
  onMove,
}: {
  tutor: TutorController;
  conversation: ConversationSummary;
  projectTitle?: string;
  switchLocked: boolean;
  organizationLocked: boolean;
  onSelected?: () => void;
  onRename: (conversation: ConversationSummary) => void;
  onMove: (conversation: ConversationSummary) => void;
}) {
  const confirm = useConfirm();

  async function remove(): Promise<void> {
    if (await confirm({
      title: "Delete this conversation?",
      description: "Only this chat is deleted. Documents, cards, study guides and review schedules stay unchanged.",
      confirmLabel: "Delete conversation",
      destructive: true,
    })) {
      await tutor.removeConversation(conversation);
    }
  }

  return (
    <div className={styles.historyRow}>
      <Button
        variant="ghost"
        className={styles.historyItem}
        aria-current={tutor.detail?.conversation.id === conversation.id ? "page" : undefined}
        disabled={switchLocked}
        onClick={() => {
          void tutor.selectConversation(conversation.id);
          onSelected?.();
        }}
      >
        <span>{conversation.title}</span>
        {projectTitle ? <small className={styles.historyMeta}>{projectTitle}</small> : null}
      </Button>
      <OverflowMenu
        label={`${conversation.title} actions`}
        disabled={organizationLocked}
        items={[
          { id: "rename", label: "Rename", onSelect: () => onRename(conversation) },
          { id: "move", label: "Move", onSelect: () => onMove(conversation) },
          {
            id: "archive",
            label: conversation.archivedAt === null ? "Archive" : "Restore",
            onSelect: () => { void tutor.setConversationArchived(conversation, conversation.archivedAt === null); },
          },
          { id: "delete", label: "Delete", destructive: true, onSelect: () => { void remove(); } },
        ]}
      />
    </div>
  );
}

export function TutorHistory({ tutor, onSelected }: { tutor: TutorController; onSelected?: () => void }) {
  const confirm = useConfirm();
  const [projectEditor, setProjectEditor] = useState<TutorProject | "new" | null>(null);
  const [renameTarget, setRenameTarget] = useState<ConversationSummary | null>(null);
  const [moveTarget, setMoveTarget] = useState<ConversationSummary | null>(null);
  const switchLocked = tutor.busy || tutor.loading || tutor.pending !== null || tutor.preparation.locked || tutor.organizing;
  const organizationLocked = tutor.loading || tutor.organizing;
  const activeProjects = tutor.projects.filter((project) => project.archivedAt === null);
  const archivedProjects = tutor.projects.filter((project) => project.archivedAt !== null);
  const conversations = tutor.detail && !tutor.conversations.some((saved) => saved.id === tutor.detail?.conversation.id)
    ? [tutor.detail.conversation, ...tutor.conversations]
    : tutor.conversations;
  const activeConversations = conversations.filter((conversation) => conversation.archivedAt === null);
  const archivedConversations = conversations.filter((conversation) => conversation.archivedAt !== null);
  const ungrouped = activeConversations.filter((conversation) => conversation.projectId === null);

  function conversationsFor(projectId: string): ConversationSummary[] {
    return activeConversations.filter((conversation) => conversation.projectId === projectId);
  }

  function projectTitle(projectId: string | null): string | undefined {
    return tutor.projects.find((project) => project.id === projectId)?.title;
  }

  async function removeProject(project: TutorProject): Promise<void> {
    if (await confirm({
      title: `Delete ${project.title}?`,
      description: "The project's conversations move to Ungrouped. Their messages and sources remain intact, and independent study materials are not deleted.",
      confirmLabel: "Delete project",
      destructive: true,
    })) {
      await tutor.removeProject(project);
    }
  }

  function ProjectGroup({ project }: { project: TutorProject }) {
    const conversations = conversationsFor(project.id);
    return (
      <section className={styles.historyGroup} aria-label={`${project.title} project`}>
        <div className={styles.historyGroupHeader}>
          <strong>{project.title}</strong>
          <OverflowMenu
            label={`${project.title} project actions`}
            disabled={organizationLocked}
            items={[
              { id: "edit", label: "Edit project", onSelect: () => setProjectEditor(project) },
              {
                id: "archive",
                label: project.archivedAt === null ? "Archive project" : "Restore project",
                onSelect: () => { void tutor.setProjectArchived(project, project.archivedAt === null); },
              },
              { id: "delete", label: "Delete project", destructive: true, onSelect: () => { void removeProject(project); } },
            ]}
          />
        </div>
        {project.archivedAt === null ? (
          <Button
            size="small"
            variant="text"
            disabled={switchLocked}
            onClick={() => {
              tutor.newConversationInProject(project.id);
              onSelected?.();
            }}
          >
            + New chat in project
          </Button>
        ) : null}
        {conversations.map((conversation) => (
          <ConversationRow
            key={conversation.id}
            tutor={tutor}
            conversation={conversation}
            switchLocked={switchLocked}
            organizationLocked={organizationLocked}
            onSelected={onSelected}
            onRename={setRenameTarget}
            onMove={setMoveTarget}
          />
        ))}
        {!conversations.length ? <p className={styles.historyMeta}>No active chats.</p> : null}
      </section>
    );
  }

  return (
    <>
      <nav className={styles.history} aria-label="Conversations">
        <div className={styles.toolbar}>
          <Button
            disabled={switchLocked}
            onClick={() => {
              tutor.newConversation();
              onSelected?.();
            }}
          >
            + New chat
          </Button>
          <Button variant="secondary" disabled={organizationLocked} onClick={() => setProjectEditor("new")}>
            New project
          </Button>
        </div>

        {activeProjects.map((project) => <ProjectGroup key={project.id} project={project} />)}

        {ungrouped.length ? (
          <section className={styles.historyGroup} aria-label="Ungrouped conversations">
            <div className={styles.historyGroupHeader}><strong>Ungrouped</strong></div>
            {ungrouped.map((conversation) => (
              <ConversationRow
                key={conversation.id}
                tutor={tutor}
                conversation={conversation}
                switchLocked={switchLocked}
                organizationLocked={organizationLocked}
                onSelected={onSelected}
                onRename={setRenameTarget}
                onMove={setMoveTarget}
              />
            ))}
          </section>
        ) : !activeProjects.length && !archivedProjects.length && !archivedConversations.length ? (
          <p className={styles.muted}>Your conversations and projects appear here.</p>
        ) : null}

        {archivedProjects.length ? (
          <Disclosure title={`Archived projects (${archivedProjects.length})`}>
            <div className={styles.historyArchive}>
              {archivedProjects.map((project) => <ProjectGroup key={project.id} project={project} />)}
            </div>
          </Disclosure>
        ) : null}

        {archivedConversations.length ? (
          <Disclosure title={`Archived chats (${archivedConversations.length})`}>
            <div className={styles.historyArchive}>
              {archivedConversations.map((conversation) => (
                <ConversationRow
                  key={conversation.id}
                  tutor={tutor}
                  conversation={conversation}
                  projectTitle={projectTitle(conversation.projectId)}
                  switchLocked={switchLocked}
                  organizationLocked={organizationLocked}
                  onSelected={onSelected}
                  onRename={setRenameTarget}
                  onMove={setMoveTarget}
                />
              ))}
            </div>
          </Disclosure>
        ) : null}
      </nav>

      {projectEditor ? (
        <ProjectEditor
          tutor={tutor}
          project={projectEditor === "new" ? null : projectEditor}
          onClose={() => setProjectEditor(null)}
        />
      ) : null}
      {renameTarget ? (
        <RenameConversationDialog tutor={tutor} conversation={renameTarget} onClose={() => setRenameTarget(null)} />
      ) : null}
      {moveTarget ? (
        <MoveConversationDialog tutor={tutor} conversation={moveTarget} onClose={() => setMoveTarget(null)} />
      ) : null}
    </>
  );
}
