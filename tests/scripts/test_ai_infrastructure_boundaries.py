import subprocess
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
VERIFIER = REPOSITORY_ROOT / "scripts" / "verify-ai-infrastructure-boundaries.mjs"


class AiInfrastructureBoundaryTests(unittest.TestCase):
    def test_accepts_complete_bounded_topology(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            self.write_valid_topology(Path(root))
            result = self.run_verifier(root)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_retired_root_monolith(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(root_path, "src-tauri/src/ai/gateway.rs", "pub struct Legacy;\n")
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("retired AI-infrastructure monolith", result.stderr)

    def test_rejects_reqwest_outside_gateway(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(
                root_path,
                "src-tauri/src/ai/providers.rs",
                "fn illegal(client: reqwest::Client) { let _ = client; }\n",
            )
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("provider HTTP belongs only", result.stderr)

    def test_rejects_tauri_commands_outside_command_owner(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(
                root_path,
                "src-tauri/src/ai/profiles/service.rs",
                "#[tauri::command]\nfn illegal(_state: tauri::State<'_, ()>) {}\n",
            )
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("profile Tauri state belongs only", result.stderr)

    def test_rejects_profile_sql_outside_profile_owner(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(
                root_path,
                "src-tauri/src/ai/document_provider.rs",
                'const QUERY: &str = "SELECT * FROM ai_provider_profiles";\n',
            )
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("profile/default SQL belongs", result.stderr)

    def test_rejects_probe_sql_outside_probe_repository(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(
                root_path,
                "src-tauri/src/ai/document_provider.rs",
                'const QUERY: &str = "SELECT * FROM ai_provider_capabilities";\n',
            )
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("probe/capability SQL belongs", result.stderr)

    def test_rejects_replacement_monolith(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-ai-infrastructure-") as root:
            root_path = Path(root)
            self.write_valid_topology(root_path)
            self.write(
                root_path,
                "src-tauri/src/ai/gateway/client.rs",
                "x" * (48 * 1024 + 1),
            )
            result = self.run_verifier(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("anti-monolith limit", result.stderr)

    def write_valid_topology(self, root: Path) -> None:
        modules = {
            "gateway": [
                "contracts.rs",
                "client.rs",
                "request.rs",
                "response.rs",
                "retry.rs",
                "errors.rs",
                "metadata.rs",
                "streaming/mod.rs",
                "streaming/sse.rs",
                "streaming/usage.rs",
            ],
            "profiles": [
                "contracts.rs",
                "commands.rs",
                "service.rs",
                "repository.rs",
                "policy.rs",
                "projections.rs",
                "defaults.rs",
            ],
            "probes": [
                "contracts.rs",
                "commands.rs",
                "runner.rs",
                "phases.rs",
                "capabilities.rs",
                "repository.rs",
            ],
        }
        self.write(
            root,
            "src-tauri/src/ai/gateway/mod.rs",
            "pub struct AiGateway; pub struct AiGatewayError; pub struct SafeAiError;\n",
        )
        self.write(
            root,
            "src-tauri/src/ai/profiles/mod.rs",
            "mod commands; pub struct ProfileSnapshot; fn load_profile_request() {}\n",
        )
        self.write(
            root,
            "src-tauri/src/ai/probes/mod.rs",
            "mod commands; pub struct AiProviderProbeResultDto; fn load_document_probe_evidence() {}\n",
        )
        for package, names in modules.items():
            for name in names:
                self.write(root, f"src-tauri/src/ai/{package}/{name}", "// bounded owner\n")

    def write(self, root: Path, relative_path: str, content: str) -> None:
        path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def run_verifier(self, root: str):
        return subprocess.run(
            ["node", str(VERIFIER), f"--root={root}"],
            cwd=REPOSITORY_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )


if __name__ == "__main__":
    unittest.main()
