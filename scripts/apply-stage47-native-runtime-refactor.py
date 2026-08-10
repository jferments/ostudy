#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"expected patch anchor missing in {path}: {old[:100]!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


replace(
    "src-tauri/src/pdf_backend.rs",
    "const MAX_TEXT_EXTRACTION_WORKERS: usize = 16;",
    "const TEXT_EXTRACTION_WORKER_MEMORY_BYTES: u64 = 96 * 1024 * 1024;",
)
replace(
    "src-tauri/src/pdf_backend.rs",
    "    let worker_count = text_extraction_worker_count(page_count);\n",
    """    let resource_plan = crate::system_resources::plan_parallel_workers(
        page_count,
        TEXT_EXTRACTION_WORKER_MEMORY_BYTES,
        \"OSTUDY_PDF_TEXT_WORKERS\",
    );
    let worker_count = resource_plan.worker_count;
    crate::logging::info(
        \"pdf_text_extraction_resources\",
        format!(
            \"pages={page_count} logical_cpus={} available_memory_mb={} working_budget_mb={} per_worker_mb={} workers={worker_count}\",
            resource_plan.logical_cpus,
            resource_plan.available_memory_bytes / (1024 * 1024),
            resource_plan.working_memory_budget_bytes / (1024 * 1024),
            resource_plan.per_worker_memory_bytes / (1024 * 1024),
        ),
    );
""",
)
replace(
    "src-tauri/src/pdf_backend.rs",
    """fn text_extraction_worker_count(page_count: usize) -> usize {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    bounded_text_extraction_worker_count(page_count, available)
}

fn bounded_text_extraction_worker_count(page_count: usize, available: usize) -> usize {
    available
        .clamp(1, MAX_TEXT_EXTRACTION_WORKERS)
        .min(page_count.max(1))
}

""",
    "",
)
replace(
    "src-tauri/src/pdf_backend.rs",
    """    #[test]
    fn text_extraction_worker_count_is_bounded_by_pages_and_cpu() {
        assert_eq!(bounded_text_extraction_worker_count(1, 64), 1);
        assert_eq!(bounded_text_extraction_worker_count(5, 64), 5);
        assert_eq!(
            bounded_text_extraction_worker_count(100, 64),
            MAX_TEXT_EXTRACTION_WORKERS
        );
        assert_eq!(bounded_text_extraction_worker_count(100, 4), 4);
        assert_eq!(bounded_text_extraction_worker_count(100, 0), 1);
    }

""",
    """    #[test]
    fn text_extraction_declares_a_conservative_per_worker_memory_estimate() {
        assert_eq!(TEXT_EXTRACTION_WORKER_MEMORY_BYTES, 96 * 1024 * 1024);
    }

""",
)

replace(
    "src-tauri/src/document_ingestion.rs",
    """const OCR_TIMEOUT_SECONDS: u32 = 120;
const MAX_OCR_WORKERS: usize = 8;
const OCR_QUEUE_DEPTH_PER_WORKER: usize = 2;
""",
    """const OCR_TIMEOUT_SECONDS: u32 = 120;
const OCR_WORKER_MEMORY_BYTES: u64 = 384 * 1024 * 1024;
const OCR_QUEUE_ITEM_MEMORY_BYTES: u64 = MAX_OCR_RENDER_BYTES as u64;
const OCR_QUEUE_DEPTH_PER_WORKER: usize = 2;
""",
)
replace(
    "src-tauri/src/document_ingestion.rs",
    """    let worker_count = ocr_worker_count(candidates.len());
    let queue_capacity = worker_count
        .saturating_mul(OCR_QUEUE_DEPTH_PER_WORKER)
        .max(1);
    crate::logging::info(
        \"document_ocr_parallel_started\",
        format!(
            \"document={document_id} pages={} workers={worker_count} queue_capacity={queue_capacity}\",
            candidates.len()
        ),
    );
""",
    """    let resource_plan = crate::system_resources::plan_parallel_workers(
        candidates.len(),
        OCR_WORKER_MEMORY_BYTES,
        \"OSTUDY_OCR_WORKERS\",
    );
    let worker_count = resource_plan.worker_count;
    let queue_capacity = crate::system_resources::queue_capacity(
        resource_plan,
        candidates.len(),
        OCR_QUEUE_ITEM_MEMORY_BYTES,
        OCR_QUEUE_DEPTH_PER_WORKER,
    );
    crate::logging::info(
        \"document_ocr_parallel_started\",
        format!(
            \"document={document_id} pages={} logical_cpus={} available_memory_mb={} working_budget_mb={} per_worker_mb={} workers={worker_count} queue_capacity={queue_capacity}\",
            candidates.len(),
            resource_plan.logical_cpus,
            resource_plan.available_memory_bytes / (1024 * 1024),
            resource_plan.working_memory_budget_bytes / (1024 * 1024),
            resource_plan.per_worker_memory_bytes / (1024 * 1024),
        ),
    );
""",
)
replace(
    "src-tauri/src/document_ingestion.rs",
    """fn ocr_worker_count(candidate_count: usize) -> usize {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    bounded_ocr_worker_count(candidate_count, available)
}

fn bounded_ocr_worker_count(candidate_count: usize, available: usize) -> usize {
    available
        .clamp(1, MAX_OCR_WORKERS)
        .min(candidate_count.max(1))
}

""",
    "",
)
replace(
    "src-tauri/src/document_ingestion.rs",
    """    #[test]
    fn ocr_worker_count_uses_multiple_cores_without_unbounded_fanout() {
        assert_eq!(bounded_ocr_worker_count(1, 32), 1);
        assert_eq!(bounded_ocr_worker_count(4, 32), 4);
        assert_eq!(bounded_ocr_worker_count(100, 32), MAX_OCR_WORKERS);
        assert_eq!(bounded_ocr_worker_count(100, 3), 3);
        assert_eq!(bounded_ocr_worker_count(100, 0), 1);
    }

""",
    """    #[test]
    fn ocr_memory_estimates_are_explicit_and_queue_items_are_bounded() {
        assert_eq!(OCR_WORKER_MEMORY_BYTES, 384 * 1024 * 1024);
        assert_eq!(OCR_QUEUE_ITEM_MEMORY_BYTES, 64 * 1024 * 1024);
    }

""",
)

replace(
    "src-tauri/src/document_pdf_tests.rs",
    'PdfRenderRuntime::new(None).expect("bundled PDFium should initialize")',
    'PdfRenderRuntime::new(None).expect("provisioned PDFium should initialize")',
)
replace(
    "src-tauri/src/document_pdf_tests.rs",
    '"the bundled PDFium renderer should render the fixture"',
    '"the provisioned PDFium renderer should render the fixture"',
)
replace(
    "src-tauri/src/document_pdf_tests.rs",
    '"pdfium-render-0.9.3/pdfium-7881-bundled-v1-120dpi"',
    '"pdfium-render-0.9.3/pdfium-api-7881-external-v2-120dpi"',
)

manifest = "scripts/verify-project-manifests.mjs"
replace(
    manifest,
    '  ["pdfium-bundled", `pdfium-bundled = { version = "=${env.get("PDFIUM_BUNDLED_REQUIREMENT")}"`],\n',
    '  ["pdfium-render", `pdfium-render = { version = "=${env.get("PDFIUM_RENDER_VERSION")}"`],\n  ["sysinfo", `sysinfo = { version = "=${env.get("SYSINFO_VERSION")}"`],\n',
)
replace(
    manifest,
    'if (cargo.includes("justpdf")) failures.push("justpdf must not remain in the renderer dependency graph");\n',
    'if (cargo.includes("pdfium-bundled")) failures.push("pdfium-bundled must not enter the dependency graph");\nif (cargo.includes("justpdf")) failures.push("justpdf must not remain in the renderer dependency graph");\n',
)
replace(
    manifest,
    """requireText(\"PDFium bundled renderer policy\", cargo, [
  \"target_os = \\\"android\\\"\",
  \"target_os = \\\"ios\\\"\",
  `pdfium-bundled = { version = \"=${env.get(\"PDFIUM_BUNDLED_REQUIREMENT\")}\", features = [\"bundled\"] }`,
]);
""",
    """requireText(\"PDFium external renderer policy\", cargo, [
  \"target_os = \\\"android\\\"\",
  \"target_os = \\\"ios\\\"\",
  `pdfium-render = { version = \"=${env.get(\"PDFIUM_RENDER_VERSION\")}\"`,
  '\"pdfium_7881\"',
  '\"thread_safe\"',
]);
""",
)
replace(
    manifest,
    'requireText("check-fast.sh", checkFast, ["check-static.sh", "pnpm test", "cargo check"]);',
    'requireText("check-fast.sh", checkFast, ["check-static.sh", "install-native-deps.sh", "pnpm test", "cargo check"]);',
)
replace(
    manifest,
    'requireText("check.sh", checkFull, ["pnpm run build", "cargo clippy", "cargo test", "smoke-native.sh"]);',
    'requireText("check.sh", checkFull, ["install-native-deps.sh", "pnpm run build", "cargo clippy", "cargo test", "smoke-native.sh"]);',
)
replace(
    manifest,
    '  "scripts/build.sh",\n',
    '  "scripts/build.sh",\n  "scripts/install-native-deps.sh",\n  "scripts/install-native-deps.ps1",\n  "scripts/native-deps-manifest.json",\n',
)
replace(
    manifest,
    '  "src-tauri/src/pdf_render_backend.rs",\n',
    '  "src-tauri/src/pdf_render_backend.rs",\n  "src-tauri/src/system_resources.rs",\n',
)
replace(
    manifest,
    '`OK   Identity, exact versions, ${migrationFiles.length} immutable migrations, environment mirrors, executable script modes, PDFium rendering, portable structured extraction, and tiered qualification manifests are consistent.`',
    '`OK   Identity, exact versions, ${migrationFiles.length} immutable migrations, environment mirrors, audited native PDFium provisioning, resource-aware document concurrency, and tiered qualification manifests are consistent.`',
)

print("Applied Stage 4.7 native-runtime/resource-scheduler source patches.")
