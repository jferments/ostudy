import subprocess
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
VERIFIER = REPOSITORY_ROOT / "scripts" / "verify-document-ai-boundaries.mjs"
REQUIRED_MODULES = [
    "context.rs",
    "contracts.rs",
    "curation.rs",
    "flashcards.rs",
    "outline.rs",
    "schemas.rs",
    "support.rs",
    "transport.rs",
]


class DocumentAiBoundaryVerifierTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.ai_root = self.root / "src-tauri" / "src" / "ai"
        self.module_root = self.ai_root / "document_intelligence"
        self.module_root.mkdir(parents=True)
        (self.ai_root / "mod.rs").write_text(
            "pub(crate) mod document_intelligence;\n", encoding="utf-8"
        )
        declarations = "\n".join(
            f"mod {name.removesuffix('.rs')};" for name in REQUIRED_MODULES
        )
        (self.module_root / "mod.rs").write_text(
            declarations + "\n", encoding="utf-8"
        )
        for name in REQUIRED_MODULES:
            (self.module_root / name).write_text("// bounded module\n", encoding="utf-8")
        (self.module_root / "transport.rs").write_text(
            "\n".join(
                [
                    "fn request_json() {}",
                    "fn record_document_usage() {}",
                    "fn prepare_document_request() {}",
                    "fn decode_document_output() {}",
                    "fn post_document_json() {}",
                ]
            )
            + "\n",
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

    def test_accepts_the_required_modular_boundary(self) -> None:
        result = self.run_verifier()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Document-AI boundaries", result.stdout)

    def test_rejects_a_legacy_monolith(self) -> None:
        (self.ai_root / "document_intelligence.rs").write_text(
            "// old monolith\n", encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("legacy document-intelligence entry point", result.stderr)

    def test_rejects_semantic_windows_in_provider_transport(self) -> None:
        transport = self.module_root / "transport.rs"
        transport.write_text(
            transport.read_text(encoding="utf-8")
            + "use crate::document_semantics::SemanticWindow;\n",
            encoding="utf-8",
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not depend on SemanticWindow", result.stderr)

    def test_rejects_provider_wire_vocabulary_in_domain_reasoning(self) -> None:
        (self.module_root / "outline.rs").write_text(
            'fn leak() { let _ = "response_format"; }\n', encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("belongs in ai/providers/", result.stderr)

    def test_rejects_provider_wire_vocabulary_in_document_transport(self) -> None:
        transport = self.module_root / "transport.rs"
        transport.write_text(
            transport.read_text(encoding="utf-8")
            + 'fn leak() { let _ = "beta/chat/completions"; }\n',
            encoding="utf-8",
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("belongs in ai/providers/", result.stderr)

    def test_rejects_a_replacement_monolith(self) -> None:
        (self.module_root / "outline.rs").write_text(
            "x" * (48 * 1024 + 1), encoding="utf-8"
        )
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("anti-monolith limit", result.stderr)

    def test_rejects_a_missing_required_module(self) -> None:
        (self.module_root / "curation.rs").unlink()
        result = self.run_verifier()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required document-intelligence module is missing", result.stderr)


if __name__ == "__main__":
    unittest.main()
