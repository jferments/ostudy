# UI visual refresh roadmap

Status: active implementation roadmap for the post-PR #96 visual pass. PR #96 completed the shadcn/Base UI/Tailwind architecture migration; this work changes ordinary presentation and removes migration-era visual compatibility while preserving product behavior.

## Design contract

The default rule is **stock first**:

> Use the owned shadcn/Base UI Nova component composition, sizing, spacing, hierarchy and ordinary variants unless oStudy has a concrete product, accessibility or platform reason to differ.

The application still owns semantic colors and all four themes, capability state, coarse-pointer touch targets, reduced motion/material behavior, safe areas/IME rules and domain semantics. Stock shadcn geometry does not make shadcn a theme authority.

Application facades remain useful when they express product meaning or platform behavior. A wrapper that only preserves the appearance of the pre-#96 UI should be removed or thinned.

## Non-goals

No scheduler, persistence, IPC, provider, knowledge/Tutor, document-processing, sync-domain, Chart.js or Tiptap schema redesign. PR #94 source permissions, optional grounding, private question criteria, citations, durable jobs and operation identities remain unchanged.

## V0 — baseline and contracts

Inventory public facades and specialized presentation. Preserve the existing 16 registered CSS owners unless a simpler implementation retains their real geometry/content requirements. Historical visual baselines remain historical evidence rather than new acceptance.

## V1 — primitive normalization

Refresh Button, Card, Badge and form presentation toward Nova. Add application-owned Card slots, Item and ButtonGroup. Standard controls use compact fine-pointer geometry; coarse input expands to the existing touch minimum. Remove vendor-only migration variants where ordinary shadcn variants plus application semantics suffice.

## V2 — abstraction cleanup

Prefer Card for bounded information, Item for list objects, Alert/notice for status, Field/FormField for forms and ButtonGroup/Toolbar for related actions. Reduce generic Surface elevation/material styling and retain it only as a compatibility/product-state boundary while consumers migrate.

## V3 — shell and global hierarchy

Use flatter Sidebar rows, a quiet inset content relationship, simpler page headers, fewer decorative icon tiles, restrained command bars and semantic typography rather than nested boxes.

## V4 — dense workflows

Refresh Settings, provider/task-model setup, sync, templates, Flashcards, Search/Filter, bulk actions and editor chrome. Preserve all existing operations, identities, drafts, import receipts, drag/reorder alternatives and sync dependencies.

## V5 — learning/content workflows

Refresh Documents, Study Guides, Tutor, Practice and Analytics so content dominates and application chrome recedes. Documents/guide lists should use Item-like rows where appropriate. Tutor transcript/composer remains the focus while sources/memory/configuration stay secondary. Practice keeps its custom two-face card geometry and scheduling semantics.

## V6 — cleanup and documentation

Delete dead presentation-only variants/wrappers after their consumers migrate. Reconcile `ui-overhaul-*` documents as completed PR #96 history, update `docs/architecture/ui.md`, vendor provenance and agent/code standards, and make this file the current UI implementation roadmap.

## V7 — visual and platform acceptance

Validation order remains focused tests -> UI policy/CSS compilation -> static -> fast -> targeted native tests -> frozen full Linux/Android qualification -> human visual/device acceptance.

Human review covers Light/Dark/Console/Synthwave, compact/medium/wide, enlarged text, keyboard focus, coarse input, reduced motion/materials, realistic empty/populated/error/busy states, editor content, Tutor, Study Guides, Practice and physical Android IME/Back behavior.

A new visual baseline is accepted only after the intended appearance is reviewed. Do not overwrite the historical Stage 0 baseline to make intentional differences disappear.
