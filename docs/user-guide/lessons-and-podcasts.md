# Lessons and podcasts

A subject's **Lessons** are a short course written from its documents, one lesson for each main topic. Each lesson is a set of lecture slides that a teacher talks through aloud, stopping now and then to ask you a question. **Podcasts** turn a subject's lessons into a conversation between two hosts to listen to. Both need an AI service (see [AI providers](ai-providers.md)); reading them aloud needs a voice, which can be a free one on your device.

<p align="center">
  <img src="../../assets/screenshots/lesson-kidney-desktop.webp" alt="A lesson's lecture: a slide about the inside of the kidney with its anatomy figure beside the points, the strip of slides on the left, Back, Next and the zoom buttons below, and Closed captions" width="860">
</p>

## Write a subject's lessons

Tick **Lessons** in **New study set** when you start a subject, in **Add materials…** on a subject you already have, or in **Make study materials** on the subject's **Documents** tab (see [Documents and AI generation](documents-and-ai-generation.md#new-study-set)). The course is planned from what ostudy already read in the subject's documents, and the lessons are written one after another; the **Lessons** tab shows **Planning the course…** and then how many are written. A document you left out of study materials is left out of the lessons too.

When **Make lecture audio with new lessons** is on in **Settings → AI** (it is on once you have a voice), each new lesson also gets its teacher's lecture written and read aloud straight away, so **Play lecture** starts at once. The tab shows a **Lecture audio progress** bar while that runs.

## The Lessons tab

The tab lists the course in order with how many lessons are done. Each lesson shows whether it is **In progress** or **Done**, and **Start lesson** or **Continue lesson**. A lesson whose documents changed since it was written says **Source changed** and offers **Update**, which writes that lesson again from the documents as they are now. **Rewrite course** writes every lesson again, at about the cost of writing the course the first time; each lesson keeps its place, and what you did in a lesson stays with its earlier version. Home shows the slide your current lesson is on, with **Resume**.

Lesson progress is only a place to pick up from. Answering a lesson's questions never counts as a flashcard review and never changes a card's schedule.

## Watch and listen to a lecture

A lesson opens as its lecture: the slide in the middle, the strip of slides beside it, and **Back** and **Next** under it. **Smaller slide** and **Larger slide** zoom the slide, and the slides follow the **Theme** you choose at the top (the study guide themes, Navy unless you change it). A slide shows the one figure from its source pages that best fits it when the document has figures.

- **Write the teacher's lecture** appears while a lesson has no lecture yet. It uses your AI service once, and the lecture is kept for that version of the lesson. Until then, each slide's own notes are read.
- **Play lecture** has the teacher talk over each slide in the teacher's own words and move on when the slide is done. **Stop** stops it, and choosing another slide stops it too. With **Autoplay** off, the lecture waits on each slide until you choose **Next**.
- Now and then the teacher stops to ask the class a question, some before a slide's talk and some after it. Answer it, and the teacher says whether you were right and goes on. ostudy marks choose-one answers itself, without the AI. **Answer the teacher's question** lets you answer it when the lecture is not playing.
- **Closed captions** shows what the teacher says under the slide.
- **Read this step** opens the lesson's own reading for the slide, and a slide with a check offers **Check yourself** (beside the slide on a wide screen, in a sheet on a phone).
- A slide's flashcards sit beside it: **Add to deck** adds them to one of the subject's decks.
- **Write on slides** opens the slides with the annotation tools, the same as on a PDF page ([Tools](pdf-annotation.md#tools)). What you write is kept with the lesson.
- **Save as .pptx** saves the lesson as an editable PowerPoint deck, with the narration as speaker notes.

Autoplay, captions, the zoom and the theme are remembered on the device.

## Make and play a podcast

On a subject's **Podcasts** tab, **Make a podcast** asks which lessons to use, the **Length** (5 to 30 minutes, 15 to start) and each host's voice. Two hosts, Ada and Ben, talk through the lessons you picked using only what those lessons say. Writing the conversation is one request to your AI service; the voices read it as it plays. A New study set with **Podcast** ticked makes one from the first three lessons once they are written.

The podcast player shows the chapters and the transcript with the line being spoken marked. Choose a line or a chapter to play from there. **Play** and **Pause**, **Back 15 seconds** and **Ahead 15 seconds**, and the speed (1×, 1.25×, 1.5× or 0.75×) are under it. **Delete podcast** in its row removes it.

## Voices

Choose the voice under **Voice** in **Settings → AI**:

- **This device (free)** reads aloud with Kokoro on your device after a one-time download. Nothing is sent anywhere.
- Through OpenRouter, **Drama 3** (the default) reads in one of your saved voices, such as **Relaxed man** or **Calm older woman**, and **Kokoro** offers its own voices. Each part costs well under a cent the first time.

Each lesson keeps one voice, and audio that was read once is kept in your library, so listening again costs nothing. Formulas and chemical notation are said in words. [AI providers](ai-providers.md#choose-a-model-for-each-task) covers saving your own voices and choosing the hosts' voices.

## Where lessons are kept

Lessons, lectures, podcasts, their audio, your progress and what you wrote on slides are part of every [library backup](backups-imports-and-sharing.md), and [device sync](device-sync.md) sends them when **Lessons** is selected. Deleting a subject deletes its lessons and podcasts. Without an AI service the Lessons and Podcasts tabs are hidden, and lessons you already have come back when you set one up.
