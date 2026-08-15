# Interface, appearance, and guidance

ostudy uses the same information architecture and study behavior across desktop, tablet, and mobile. Appearance settings change presentation, not domain behavior.

## Themes

The built-in themes are **Light**, **Dark**, **Console**, and **Synthwave**. Every theme implements the same semantic roles for canvas, surfaces, text, borders, accent, status, focus, elevation, and materials. Feature screens do not select theme-specific colors or branch their behavior by theme name.

## Typography and scale

The interface font and default card font can be selected independently. Interface scale and card text size are also separate so navigation can remain compact while study material stays comfortable to read.

Installed custom font-family names are accepted as local preferences. Font files are not copied into the library or included in exports.

## Motion

Motion can follow the operating-system preference, use the full qualified experience, use reduced motion, or disable nonessential motion.

Reduced or disabled motion removes spatial route movement, card-stack entrance movement, card flipping, parallax, and particle celebrations. Essential state changes, progress, labels, focus, and feedback remain available without animation.

## Material effects

Material profiles control translucency, blur, lighting, texture, and depth:

- **Automatic** uses the standard cross-platform treatment while respecting reduced-transparency and forced-colors preferences.
- **Rich** enables the full qualified material treatment on capable devices.
- **Simple** uses opaque surfaces and restrained texture while retaining hierarchy through borders and elevation.

These profiles do not create separate platform implementations. The same components resolve to an appropriate material through semantic tokens and capability-aware root attributes.

## Decorative artwork

Decorative artwork can be disabled independently of functional icons and educational content. Hiding decorative artwork never hides images inserted into flashcards, diagrams from documents, controls, status information, or actions. Illustrated states retain complete text and icon fallbacks.

## Celebrations and feedback

Review and completion screens emit semantic events such as **answer revealed**, **review committed**, and **session completed**. A central experience layer interprets those events according to preferences and platform capabilities.

A completed session may produce a brief capped particle effect, haptic pulse, or audio cue. Reduced motion removes the particle effect. Disabling celebrations does not remove the completion dashboard or session analytics.

## Contextual help

Help topics are registered by stable identifiers. A help topic can provide:

- a short desktop tooltip;
- a touch-friendly help sheet;
- a related user-guide label;
- a link into an applicable guided tutorial.

Help overlays close before ordinary navigation when Escape or Android Back is used.

## Guided tutorials

Tutorials are short contextual modules rather than a long blocking onboarding sequence. A module starts only when its semantic target exists on the current screen. Tutorial progress is versioned, device-local presentation state and is not included in complete-library backups.

Tutorials can be skipped, restarted from contextual help, or disabled. They never modify decks, cards, scheduling, analytics, providers, credentials, or managed files.
