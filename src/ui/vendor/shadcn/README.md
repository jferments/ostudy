# Owned shadcn components

Upstream baseline: shadcn-ui/ui commit `3ba91b1cc83e1bbe4ab35a422ff2a694849c5048`.
License: MIT; the distributed notice is [SHADCN-MIT.txt](../../../../public/licenses/SHADCN-MIT.txt).

The Base UI family in `apps/v4/registry/bases/base/ui/` supplies the composition patterns for Button, Badge, Skeleton, Input, NativeSelect, Checkbox, Switch, RadioGroup, Slider, Textarea, Collapsible, Tabs, Card, Dialog, DropdownMenu, Tooltip, Toast, Alert, Progress, Empty and Sidebar. Form renderers are grouped in `form-controls.tsx`; modal/notification files adapt the dialog/toast patterns to oStudy's facade APIs.

This directory is the only Base UI import owner. Features import oStudy facades, not copied implementations or vendor prop types. Native structural elements inside these implementations are normal shadcn composition, not a claim that every div needs a behavioral primitive.

## Deliberate adaptations

- Registry `cn-*` placeholders are expanded to semantic `tw:` utility recipes. oStudy supplies colors, radii, fonts, motion, focus and touch dimensions; the upstream default palette is not another theme authority.
- Button retains Base UI and CVA composition while preserving the existing primary/destructive/outline/ghost/back appearances, native refs/forms and loading action names.
- Badge retains mergeProps/useRender and span semantics with application status tones. Skeleton retains span callers and disables its pulse for reduced/no motion.
- TextField uses Base UI Input behind native React props. NativeSelect intentionally preserves OS picker, options/optgroups, multiple selection and numeric listboxes; listboxes do not show a misleading dropdown arrow. Labels, validation and secrets stay outside the vendor layer.
- Checkbox, Switch, RadioGroup and Slider use upstream state/interaction rather than custom event imitation. Tabs use roving focus and automatic activation. SegmentedControl's existing Home/End contract is supplied by its semantic facade using public rendered controls.
- Disclosure content stays mounted to preserve drafts. FormField keeps help actions outside the input label. Input text does not inherit small bold label styling.
- Card/Sidebar use composable structural elements. The sidebar is intentionally non-collapsing: LayoutModeProvider owns responsive behavior, so there is no second cookie, mobile detector or vendor navigation hotkey.
- ModalView uses Base UI Dialog for browser behavior. oStudy owns application overlay registration/native Back, width and compact/fullscreen presentation, busy dismissal and explicit focus policy. InlineWorkspace remains outside modal behavior.
- Dropdown menus close before a chosen action opens a new dialog. Compact actions render through the same shared Sheet policy. Tooltip behavior uses upstream positioning rather than manual screen coordinates.
- NotificationView uses Base UI Toast with one visible message, preserving the existing app API. The facade owns absolute Undo expiry and asynchronous action identity; ordinary success remains status, not an error alert.
- Standard progress uses the upstream behavior; oStudy still supports specialized rich inline progress content. Unknown totals use indeterminate feedback rather than fabricated counts. Empty/Alert retain application wording, severity and recovery actions.
- `cn` uses clsx/tailwind-merge with the configured prefix and semantic font sizes. Partial-border recipes explicitly clear other edges because Preflight is intentionally absent.

The editor Toolbar uses the pinned Base UI 1.6.0 Toolbar composition with the owned shadcn Button. It delegates roving focus and disabled behavior upstream; native font/symbol/color pickers are outside command roving so their arrow-key editing is not intercepted. ColorField is a native authored-content picker behind the Input adapter; it is not a second application palette.

## Updating and qualification

These are customized source copies, not an npm package that automatically updates the application's controls. Compare a pinned upstream revision, review behavior/dependencies, port relevant changes and record adaptations. Do not blindly run CLI overwrite against owned source.

`components.json` records family, aliases and prefix. Runtime needs no CLI, registry, CDN, hosted font or external provider to render the UI. The app's actual provider services remain unrelated to UI dependencies.

Use behavior and CSS-compilation tests before wider native qualification. Utilities temporarily compile into the existing components layer so unmigrated feature overrides remain compatible. Move them to the final layer only after auditing the remaining consumers, not by forcing global important declarations.

The generated CSS contract discovers shared UI and live Phase 4–9 sources, verifies every literal utility and all themes/capabilities. It is not a screenshot test or physical-device acceptance. Actual native focus/IME/Back, layout and human visual review remain separate requirements.
