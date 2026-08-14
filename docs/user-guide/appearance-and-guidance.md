# Appearance, accessibility, help, and guided tutorials

ostudy uses one semantic visual system across desktop, tablet, Android, and future mobile targets. Themes change the visual values assigned to surfaces, borders, text, status roles, and accents; they do not change where features live or how study data behaves.

## Themes and typography

The Appearance settings provide Light, Dark, Console, and Synthwave themes. Each theme implements the same complete semantic contract, so a screen cannot accidentally become functionally different under another theme.

Interface and flashcard typography are configured separately. Interface scale and default card text size also remain separate: increasing the interface scale does not rewrite card content, and changing the card default does not enlarge navigation or dialogs.

## Motion and material effects

Motion has three effective levels:

- **Full** uses route transitions, card flips, small interaction feedback, and optional completion celebration.
- **Reduced** removes spatial movement and particle effects while preserving immediate state changes and focus movement.
- **None** disables nonessential animation completely.

The system preference can be followed automatically or overridden in Appearance settings. Material complexity is independent of motion. Rich materials may use translucency, blur, fine grain, and restrained glow; Simple materials replace those treatments with opaque surfaces, borders, and elevation. Reduced-transparency and forced-colors preferences always take precedence over decorative effects.

## Decorative artwork and study content

The Decorative artwork setting controls mascot illustrations and nonessential empty-state or completion artwork. Every illustrated state retains its title, explanatory text, action, and functional icon when artwork is disabled.

Images that are part of a flashcard or imported study document are educational content. They are never hidden by the decorative-artwork preference.

## Contextual help

Question-mark controls provide brief hover or keyboard-focus descriptions on pointer devices. Activating the control opens a responsive help sheet with the full explanation. The same topic registry supplies both presentations, so the short and detailed descriptions cannot drift apart.

Help sheets may also offer a related guided tutorial. They reference the corresponding user-guide topic, but they do not open external pages or require a network connection.

## Guided tutorials

Tutorials are short, contextual modules that begin only when their semantic target is present. Current modules cover:

- Flashcards workspace orientation;
- Search filters and study-state scope;
- practice card reveal and confidence rating;
- document-to-flashcard generation.

Tutorial progress is device-local presentation state. It is not library content, is not included in `.ostudybackup`, and can be dismissed or reset without affecting decks, cards, schedules, analytics, or credentials.

Escape and Android Back close the current tutorial before ordinary route navigation occurs. Focus returns to the control that opened the tutorial. Tutorials never auto-start during automated tests, and disabling tutorials stops all automatic starts while preserving explicit help.

## Completion feedback

Submitting a review or finishing a session emits a semantic feedback event. A central experience layer decides whether that event becomes a visual pulse, haptic response, audio cue, or no effect according to preferences and platform capability. The domain code does not select animation values or platform-specific effects.

Completion particles are deliberately bounded and one-shot. Reduced motion or disabled celebrations remove the particles and glow without hiding the completion summary or its analytics.
