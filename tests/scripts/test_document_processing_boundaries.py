import subprocess
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
VERIFIER = REPOSITORY_ROOT / "scripts" / "verify-document-processing-boundaries.mjs"
PACKAGES = [
    "documents",
    "document_ingestion",
    "anydoc_document_adapter",
    "pdf_document_ingestion",
    "pdf_backend",
]


class DocumentProcessingBoundaryVerifierTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source_root = self.root / "src-tauri" / "src"
        self.source_root.mkdir(parents=True)
        (self.source_root / "lib.rs").write_text(
            "\n".join(f"mod {name};" for name in PACKAGES) + "\n",
            encoding="utf-8",
        )
        for name in PACKAGES:
            package = self.source_root / name
            package.mkdir()
            (package / "mod.rs").write_text("// package facade\n", encoding="utf-8")
        (self.source_root / "document_ingestion" / "contracts.rs").write_text(
            "\n".join(
                [
                    "use crate::document_canonical::CanonicalIngestion;",
                    "use crate::document_domain::DocumentFormat;",
                    "trait IngestionProgressSink {}",
                    "trait DocumentAdapter { fn supports(&self, format: DocumentFormat) -> bool; }",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (self.source_root / "document_ingestion" / "registry.rs").write_text(
            "fn route(format: DocumentFormat) { match format { DocumentFormat::Pdf => {}, DocumentFormat::Docx => {}, DocumentFormat::Csv => {}, _ => {} } }\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_verifier(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(VERIFIER), f"--root={self.root}"],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_physical_packages_and_explicit_routing(self) -> None:
        result = self.run_verifier()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Document processing", result.stdout)

    def test_rejects_a_retired_root_facade(self) -> None:
        (self.source_root / "documents.rs").write_text("// facade\n", encoding="utf-8")
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("retired root monolith/facade", result.stderr)

    def test_rejects_a_path_attributed_alias(self) -> None:
        (self.source_root / "lib.rs").write_text(
            '#[path = "document_adapters.rs"]\nmod document_ingestion;\n'
            + "\n".join(f"mod {name};" for name in PACKAGES if name != "document_ingestion")
            + "\n",
            encoding="utf-8",
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("path-attributed alias", result.stderr)

    def test_rejects_anydoc_types_outside_the_backend_package(self) -> None:
        (self.source_root / "documents" / "repository.rs").write_text(
            "use anydoc::model::Document;\n", encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("third-party AnyDoc types", result.stderr)

    def test_rejects_tauri_state_outside_document_commands(self) -> None:
        (self.source_root / "documents" / "repository.rs").write_text(
            "use tauri::State;\n", encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ordinary Rust dependencies", result.stderr)

    def test_rejects_first_supporting_backend_selection(self) -> None:
        (self.source_root / "document_ingestion" / "registry.rs").write_text(
            "fn route(format: DocumentFormat) { adapters.into_iter().find(|adapter| adapter.supports(format)); }\n",
            encoding="utf-8",
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("first-supporting-backend", result.stderr)

    def test_rejects_a_replacement_monolith(self) -> None:
        (self.source_root / "pdf_backend" / "extraction.rs").write_text(
            "x" * (64 * 1024 + 1), encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("anti-monolith limit", result.stderr)


if __name__ == "__main__":
    unittest.main()
