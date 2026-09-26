# Deferred lessons, retained narration, and PowerPoint

Status: **planning reference only**. [PR #105](https://github.com/jferments/ostudy/pull/105) remains deferred; no lesson, narration or PowerPoint implementation is introduced by PR #108.

The full preserved specification is on the deferred branch at [planning commit c53c1f720c51ebba4df9aa03b130eb184b00be7b](https://github.com/jferments/ostudy/blob/c53c1f720c51ebba4df9aa03b130eb184b00be7b/docs/development/lk-followups/lessons-narration.md). This pointer makes that unmerged plan reachable from the maintained documentation without copying the branch's older implementation assumptions into current architecture.

The retained scope is LK-10 responsive text-first lessons, LK-11 retained narration/TTS, and LK-12 local editable PowerPoint export. STT, voice cloning, speech grading, video export, a general slide editor, and hidden provider spending are outside that plan.

The production-baseline work formerly called LK-13 was completed by merged PR #106; see its [completion record](consolidation-qualification.md). When #105 is explicitly resumed, integrate the then-current main, recheck owner/provider/schema contracts, and extend the release baseline through reviewed forward migrations with preservation coverage. Reconcile this pointer with the preserved specification at that time rather than restoring the old branch wholesale.

The [hardening register](../post-106-hardening.md) records the separately planned post-#108 reliability changes. Neither that register nor this pointer authorizes implementation of the deferred lesson features.
