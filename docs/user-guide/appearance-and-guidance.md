# Appearance, accessibility, help, and tutorials

Open **Settings → Appearance**. Presentation preferences change how ostudy looks and responds, not where your study data is stored or how reviews are scheduled. Theme and typography changes preview immediately; use **Save preferences** when shown to keep them. Provider profiles and card-template edits have their own save paths.

## Navigation and available space

The same application resolves compact, medium, and wide layouts from its available viewport. Wide and medium layouts show the five destinations (Home, Library, Review, Tutor, and Progress) with labels in a rail down the left side, with Search and Settings at its bottom. Compact layouts put the destinations in a bottom bar and keep Search and Settings in the top bar, which also names the current destination.

An open Study Guide stays inside the application shell, with its own contents/article layout and close action. Smaller screens stack content rather than adding a modal that hides main navigation. On desktop the window uses your operating system's own title bar and controls; use them, or your window manager's shortcuts, to maximize or go full screen.

Compact spacing keeps keyboard focus, accessible names, and full-size touch targets. A truncated row preview does not mean stored card text was shortened; open the content to inspect it fully.

## Themes and typography

The built-in themes are **Light**, **Dark**, **Console**, and **Synthwave**. They share the same functional controls and semantic roles. Choosing a theme does not select a different study workflow.

Interface font and default card font are independent. Interface scale and default card text size are also independent, so study material can be larger without unnecessarily enlarging navigation. Current controls expose interface scale from 80–140% and default card text from 12–32 pixels; explicit formatting within card content remains a separate concern.

New installations use **IBM Plex Sans** for both default font choices. Older untouched defaults can adopt it, while explicit selections—including an explicitly chosen system font—are preserved. The installed app uses a local bundled face and falls back to the device's sans-serif stack when it is unavailable; it does not fetch a runtime web font.

A custom font-family preference relies on a font installed on the device. It does not copy that font file into the library or make it portable to another device. **Restore appearance defaults** is not a reset of review history, backup configuration, or AI profiles.

## Motion

Motion choices are **Follow system**, **Full motion**, **Reduced motion**, and **No nonessential motion**. Reduced/disabled motion removes nonessential spatial transitions, flips, parallax, and particle effects while preserving labels, progress, focus, and state changes.

Every theme uses flat, opaque surfaces without blur, translucency or texture, so there is no separate material-effects setting. Hierarchy comes from borders, surfaces and shadows, which also suits operating-system reduced-transparency and forced-colors preferences.

An animation is not the signal that a review was saved. The saved review is what counts; effects follow it and may be turned off or unavailable.

## Artwork and completion feedback

**Decorative artwork** controls nonessential mascot and illustrated-state art. Functional icons, notices, controls, educational images in cards/documents, and the raccoon brand mark beside the ostudy name remain visible. **Celebrations** permits brief one-shot completion effects when full motion is on; turning it off does not remove **Session complete** or Progress.

Platform feedback can vary by capability and preference. A device that cannot play an audio, haptic, or visual effect still studies normally. There is no separate Comfortable/Compact deck-density preference in the current interface.

## Contextual help

Help buttons (an **i** icon) sit beside some settings and notices, such as **Motion** in Settings → Appearance and the notice on Documents when no AI provider is set up. Hovering over or focusing one shows a short explanation; choosing it opens a sheet with the full explanation and the name of the guide section that covers the topic. Short and full explanations come from the same help topics rather than separately written screen text.

A topic that has a tutorial also offers **Start guided tutorial**. Help works without an AI provider and without turning tutorials on, and reading help never sends library content to a model.

## Explicitly started tutorials

**Interactive tutorials** in Settings → Appearance is off by default. To use a tutorial, turn it on, open a related Help topic, and choose **Start guided tutorial**. Each step points at a part of the screen; use **Next**, **Back**, and **Finish** to move through it, **More help** to open the step's help topic, and **Skip tutorial** or **×** to leave. If the part a step describes is not visible in the current layout, the step says so and you can continue.

Tutorials never start on their own: not when you open a screen, upgrade, or restart the app.

Escape or Android Back closes an active tutorial before ordinary navigation. Turning tutorials off closes an active tutorial and prevents new ones until you turn them on again. Which tutorials you finished or skipped is remembered on this device only; it is not library content, is not included in `.ostudybackup`, and does not change cards, schedules, providers, or credentials.

## Other Settings sections

Settings has six sections: Appearance, Review, Card templates, AI, Backup and sync, and Diagnostics. On a wider screen they are listed in a shaded pane beside the open section; on a phone Settings opens on the list, and Back returns to it from a section. Card templates lists **Built-in templates**, where you choose which formats AI writes, above **Your templates**, a full editing workspace. AI holds only **Services** (the connected AI services and their keys) and **Default for each task**. Backup and sync owns recovery and trusted-device transfer; Diagnostics owns local diagnostic configuration and troubleshooting information.

Changes to Appearance, Review and Backup and sync preview at once. While any are unsaved, a bar at the bottom offers **Discard** and **Save**. **Restore defaults** in a section's header puts that section's preferences back to their defaults, still unsaved until you choose Save.

For current study routes see [Getting started](getting-started.md), for deck actions see [Library Workbench](flashcards-workspace.md), and for recoverable data versus device-local presentation state see [Backups](backups-imports-and-sharing.md).
