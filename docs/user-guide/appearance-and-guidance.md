# Appearance, accessibility, help, and tutorials

Open **Settings → Appearance**. Presentation preferences change how ostudy looks and responds, not where your study data is stored or how reviews are scheduled. Theme and typography changes preview immediately; use **Save preferences** when shown to keep them. Provider profiles and card-template edits have their own save paths.

## Navigation and available space

The same application resolves compact, medium, and wide layouts from its available viewport. Wide/medium layouts use labeled navigation. Compact layouts show Flashcards, Study Guides, Documents, and Analytics in bottom navigation, with Settings in the upper-right gear.

An open Study Guide stays inside the application shell, with its own contents/article layout and close action. Smaller screens stack content rather than adding a modal that hides main navigation. The application uses native operating-system window chrome on desktop; old instructions for separate in-app maximize/full-screen buttons no longer describe the current shell. Use your window manager's supported controls and shortcuts.

Compact visual spacing must preserve keyboard focus, accessible names, and touch targets. A truncated row preview does not mean stored card text was shortened; open the content to inspect it fully.

## Themes and typography

The built-in themes are **Light**, **Dark**, **Console**, and **Synthwave**. They share the same functional controls and semantic roles. Choosing a theme does not select a different study workflow.

Interface font and default card font are independent. Interface scale and default card text size are also independent, so study material can be larger without unnecessarily enlarging navigation. Current controls expose interface scale from 80–140% and default card text from 12–32 pixels; explicit formatting within card content remains a separate concern.

New installations use **IBM Plex Sans** for both default font choices. Older untouched defaults can adopt it, while explicit selections—including an explicitly chosen system font—are preserved. The installed app uses a local bundled face and falls back to the device's sans-serif stack when it is unavailable; it does not fetch a runtime web font.

A custom font-family preference relies on a font installed on the device. It does not copy that font file into the library or make it portable to another device. Restore appearance defaults is not a reset of review history, backup configuration, or AI profiles.

## Motion and materials

Motion choices are **Follow system**, **Full motion**, **Reduced motion**, and **No nonessential motion**. Reduced/disabled motion removes nonessential spatial transitions, flips, parallax, and particle effects while preserving labels, progress, focus, and state changes.

Material effects are independent: **Automatic**, **Rich materials**, and **Simple opaque materials**. Simpler materials reduce translucency/blur and retain hierarchy through borders and surfaces. Operating-system reduced-transparency or forced-colors needs take precedence over optional decoration.

Do not interpret an animation as the authoritative signal that a review committed. The native result owns that fact; effects follow semantic events and may be disabled or unavailable.

## Artwork and completion feedback

**Decorative artwork** controls nonessential mascot and illustrated-state art. Functional icons, notices, controls, and educational images in cards/documents remain visible. **Celebrations** permits brief one-shot completion effects when the motion policy allows them; disabling it does not remove the completion summary or analytics.

Platform feedback can vary by capability and preference. Unsupported audio/haptic/visual effects must not prevent ordinary studying. There is no separate Comfortable/Compact deck-density preference in the current interface.

## Contextual help

Question-mark controls provide a short hover/focus explanation where applicable. Activating Help opens its touch-friendly explanation. Short and full explanations come from the same topic registry rather than separately maintained screen copy.

Help can identify a related guide or offer **Start guided tutorial**. Ordinary help is available without an AI provider or tutorial opt-in; reading help does not send library content to a model.

## Explicitly started tutorials

**Interactive tutorials** default off. To use one, enable the setting, open a relevant Help topic, and explicitly start its tutorial. Current modules cover Flashcards orientation, Search filters, practice reveal/rating, and document generation.

Tutorials never auto-start because you opened a route, exposed a target, upgraded a version, or restarted the app. Observing the target of an already-running step only keeps its spotlight aligned with layout changes.

Escape or Android Back dismisses active guidance before ordinary navigation. Turning tutorials off closes an active tutorial and prevents new starts until re-enabled. Versioned tutorial progress/dismissals are device-local presentation state, not library content included in `.ostudybackup`. They do not change cards, schedules, providers, or credentials.

## Other Settings sections

The normal tabs are Appearance, Backup/Sync, AI/LLM, Review/Scheduling, Card templates, and Diagnostics. Card templates is a full editing workspace, not a hidden advanced shortcut. Backup/Sync owns recovery and trusted-device transfer; Diagnostics owns local diagnostic configuration and troubleshooting information.

For current study routes see [Getting started](getting-started.md), for deck actions see [Flashcards](flashcards-workspace.md), and for recoverable data versus device-local presentation state see [Backups](backups-imports-and-sharing.md).
