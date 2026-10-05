# Artwork provenance and licensing

The semantic artwork registered under `src/ui/artwork` is original project artwork created for ostudy. It is distributed under the same MIT license as the application source code unless a future entry explicitly states otherwise.

## Registered semantic slots

| Slot family | Purpose | Source and license |
| --- | --- | --- |
| `empty.*` | Empty libraries, folders, documents, search results, and configuration states | Original ostudy vector artwork; MIT |
| `practice.*` | Practice preparation and completion states | Original ostudy vector artwork, and the project mascot for completion (see below); MIT |
| `brand.*` | Responsive ostudy brand and mascot treatments | Original ostudy mascot artwork (see below); MIT |
| `status.*` | Nonessential success and milestone decoration | Original ostudy vector artwork; MIT |

## Brand mark

`brand.logo.compact` shows the raccoon's head from the project's own mascot art, `assets/images/racoon mascot (logo).png` (1254 x 1254). The head-and-shoulders square is the 740 x 740 region at +232+88 of that file (inside the 840 x 800 head crop at +186+40), so the ears sit at the top and the body runs off the bottom edge. It ships as `images/raccoon-head-64.webp` and `images/raccoon-head-128.webp` (transparent WebP, downscaled with Lanczos, metadata stripped). The brand mark always shows; the decorative-artwork preference does not apply to brand slots.

`progress.coach` reuses the head files beside the Progress summary. `practice.complete` shows the whole mascot at the desk from the same source file, as `images/raccoon-study-240.webp` and `images/raccoon-study-480.webp` (transparent WebP, downscaled with Lanczos, metadata stripped). Both are decorative, so the decorative-artwork preference hides them.

Functional icons are not decorative artwork. They come from the app's icon registry (`src/ui/icons/Icon.tsx`, Lucide glyphs under the ISC notice in `public/licenses/LUCIDE-ISC.txt`) and remain visible when decorative artwork is disabled.

User-authored educational images, imported document pages, and images embedded in flashcards are content, not application artwork. The decorative-artwork preference never hides or changes them.

Any future third-party asset must be listed here with its exact filename or semantic slot, creator, source, license, and required attribution before it is registered or shipped.
