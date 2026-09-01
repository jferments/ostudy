# Interface, appearance, and guidance

ostudy uses the same information architecture and study behavior across desktop, tablet, and mobile. Appearance settings change presentation, not domain behavior.

## Responsive layout modes

The application resolves one shared shell mode from the available CSS viewport:

- **Compact** uses the phone app bar, bottom navigation, and single-column workspace layouts.
- **Medium** is intended for tablets and compact desktop windows. It uses a narrow but labeled sidebar and lets each workspace adapt to its actual content width.
- **Wide** uses the full sidebar and wide desktop layouts.

The shell publishes the current layout mode and pointer mode to the application root. Feature layouts use container queries where their available width can differ from the overall viewport because of navigation chrome. This prevents tablets from receiving an icon-only shell while their content independently switches between unrelated phone and desktop breakpoints.

Layout behavior never depends on a device name or user-agent string. Portrait and landscape tablets, desktop windows, touchscreens, and mouse-driven systems are resolved from available width, pointer precision, text scale, material preference, and motion preference.

Compact visual spacing does not reduce interaction targets. Coarse-pointer drag handles, checkboxes, and destructive or primary actions retain touch-sized controls.

## Desktop window controls

Supported desktop builds provide native **Maximize window** / **Restore window**
and **Enter full screen** / **Exit full screen** buttons. They appear at the
bottom of the sidebar in wide and medium layouts and move into the app bar when
the same desktop window becomes compact. The buttons can be reached and
activated with the keyboard, and their names and pressed state follow the real
native window state, including changes made through the operating system.

Android, other mobile builds, and browser-only previews do not show these
controls. ostudy does not imitate them by resizing browser content or invoking a
browser-only full-screen mode. Full viewport use by every route and dialog is
not simulated: after resize, maximize, or full screen, the shell and maintained
workspaces expand to the available application viewport. Dialogs stay within
the safe dynamic viewport, and workspace-style dialogs can use the full canvas
while keeping their content independently scrollable.

Study Guides use that workspace presentation while keeping the guide text at a
comfortable reading width. On compact layouts the guide becomes full-screen,
its contents and article stack vertically, and sticky editing actions remain
above safe areas and the on-screen keyboard.

## Themes

The built-in themes are **Light**, **Dark**, **Console**, and **Synthwave**. Every theme implements the same semantic roles for canvas, surfaces, text, borders, accent, status, focus, elevation, and materials. Feature screens do not select theme-specific colors or branch their behavior by theme name.

Every layout mode preserves explicit text and border contrast rather than depending only on subtle background differences. This is particularly important for color e-ink displays and other low-refresh or reduced-contrast devices.

## Typography and scale

The interface font and default card font can be selected independently. Interface scale and card text size are also separate so navigation can remain compact while study material stays comfortable to read.

New installations use **IBM Plex Sans** for both choices. An existing explicit
font selection is preserved, including an explicit choice of the system font;
only the untouched system defaults from older installations are updated to IBM
Plex Sans. Choosing **Restore appearance defaults** returns both font choices
to IBM Plex Sans without resetting review, backup, or AI settings.

IBM Plex Sans is included in installed application packages and does not need a
network connection while ostudy runs. If its local font file is missing or
unreadable, ostudy uses the device's sans-serif system stack so the interface
and cards remain usable. The exact fallback glyph metrics can vary by operating
system.

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
- an explicit action to start an applicable guided tutorial.

Help overlays close before ordinary navigation when Escape or Android Back is used.

## Interactive tutorials

Tutorials are optional guided modules rather than a long blocking onboarding sequence. New installations default them off. When **Interactive tutorials** is enabled, a module starts only after the user explicitly requests it, normally from the related Help sheet.

No tutorial is launched by navigation, by a semantic target appearing in the DOM, by a version change, or by application startup. The tutorial system may observe the target of an already-active step so its spotlight follows responsive/layout changes; that observer is not an auto-start mechanism.

Tutorial progress and dismissal state are versioned, device-local presentation state and are not included in complete-library backups. Existing stored progress remains readable. Tutorials can be skipped or explicitly restarted from Help without modifying decks, cards, scheduling, analytics, providers, credentials, or managed files.

Escape and Android Back dismiss an active tutorial before ordinary route navigation. Turning Interactive tutorials off closes an active tutorial and prevents new tutorial starts until the setting is enabled again.
