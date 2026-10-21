import { createOperationId } from "./common";
import type { LessonStepNarration } from "./lessons";
import { invoke } from "./transport";

/** The shortest, longest and default podcast, in minutes. */
export const PODCAST_MINUTES = { min: 5, max: 30, default: 15 } as const;
/** The most lessons one podcast talks through. */
export const PODCAST_MAX_LESSONS = 12;
/** About how many characters the voices read for each minute of podcast. */
export const PODCAST_CHARACTERS_PER_MINUTE = 900;

export interface PodcastSummary {
  id: string;
  subjectId: string;
  title: string;
  targetMinutes: number;
  /** How long the script takes to speak. */
  minutes: number;
  lessonTitles: string[];
  createdAt: number;
}

export interface PodcastHost {
  name: string;
  /** The speech setting's voice the host speaks in. */
  voice: string;
}

export interface PodcastTurn {
  /** Names the turn when it is spoken. */
  key: string;
  /** Which of the podcast's hosts speaks. */
  host: number;
  text: string;
}

/** One lesson the hosts talk through. */
export interface PodcastChapter {
  lessonId: string;
  title: string;
  turns: PodcastTurn[];
}

export interface Podcast {
  id: string;
  subjectId: string;
  title: string;
  targetMinutes: number;
  minutes: number;
  hosts: PodcastHost[];
  chapters: PodcastChapter[];
  createdAt: number;
}

/** A subject's podcasts, newest first. */
export async function listSubjectPodcasts(subjectId: string): Promise<PodcastSummary[]> {
  return invoke<PodcastSummary[]>("list_subject_podcasts", { input: { subjectId } });
}

export async function getPodcast(podcastId: string): Promise<Podcast> {
  return invoke<Podcast>("get_podcast", { input: { podcastId } });
}

export async function deletePodcast(podcastId: string): Promise<void> {
  return invoke<void>("delete_podcast", { input: { podcastId } });
}

/**
 * Writes a podcast about the lessons, in their order, with the default
 * document model and keeps it. `voices` are the two hosts' voices for the
 * speech model in use; the default pair when null.
 */
export async function writePodcast(
  subjectId: string,
  lessonIds: string[],
  targetMinutes: number,
  voices: [string, string] | null = null,
  operationId: string = createOperationId(),
): Promise<Podcast> {
  return invoke<Podcast>("write_podcast", {
    input: { subjectId, lessonIds, targetMinutes, voices, operationId },
  });
}

/**
 * Speaks one turn in its host's voice and keeps the audio, so playing it
 * again uses what was kept.
 */
export async function narratePodcastTurn(podcastId: string, turnKey: string): Promise<LessonStepNarration> {
  return invoke<LessonStepNarration>("narrate_podcast_turn", {
    input: { podcastId, turnKey },
  });
}
