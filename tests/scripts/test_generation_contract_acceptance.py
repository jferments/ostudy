"""Comparator regressions; ALL fixtures are fabricated, never acceptance evidence."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/generation-contract-acceptance.py"
SPEC = importlib.util.spec_from_file_location("generation_acceptance", SCRIPT)
ACCEPTANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ACCEPTANCE)


class GenerationAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.manifest_path = self.root / "manifest.json"
        self.manifest = {
            "schemaVersion": 1, "baselineSha": "a" * 40, "candidateSha": "b" * 40,
            "limits": {"maxInputOutputTokenRatio": 1.0, "maxElapsedRatio": 1.0, "maxFirstUsableRatio": 1.0},
            "cases": [self.make_case(kind) for kind in sorted(ACCEPTANCE.KINDS)],
        }

    def write_file(self, name, value, *, plain=False):
        raw = value.encode() if plain else json.dumps(value).encode()
        (self.root / name).write_bytes(raw)
        return {"path": name, "sha256": hashlib.sha256(raw).hexdigest()}

    def make_case(self, kind):
        source = self.write_file(f"{kind}-source.txt", "Synthetic source fixture.", plain=True)
        configuration = self.write_file(f"{kind}-config.json", {"fixture": True, "kind": kind})
        case = {"id": kind, "kind": kind, "sources": [source],
                "configuration": configuration, "objectiveIds": ["core", "qualified_case"],
                "artifactKinds": ["cards", "guide"],
                "regime": "resume" if kind == "collection" else "fresh"}
        for side, sha, tokens, duration in (("baseline", "a" * 40, 100, 1000),
                                           ("candidate", "b" * 40, 80, 800)):
            operation_id = f"{kind}_{side}"
            usage = {"schemaVersion": 1, "operationId": operation_id,
                     "operation": {"id": operation_id, "state": "complete",
                                   "startedAt": 100, "finishedAt": 100 + duration},
                     "requests": [{"requestId": f"{operation_id}_request",
                                   "operationId": operation_id,
                                   "provider": "fixture", "model": "fixture-model",
                                   "stage": "guide_authoring", "attempt": 1,
                                   "inputTokens": tokens, "outputTokens": 20,
                                   "cachedInputTokens": 0, "reasoningTokens": 5,
                                   "status": "response", "startedAt": 100, "finishedAt": 200}],
                     "localReuse": []}
            artifacts = [{"kind": artifact_kind, "usableAt": 100 + duration // 2, **self.write_file(
                f"{kind}-{side}-{artifact_kind}.json", {"synthetic": True, "side": side})}
                for artifact_kind in case["artifactKinds"]]
            case[side] = {
                "headSha": sha, "evidenceKind": "synthetic", "cacheState": "cold",
                "generationRegime": case["regime"],
                "operationId": operation_id,
                "sourceSetSha256": ACCEPTANCE.identity([source["sha256"]]),
                "configurationSha256": configuration["sha256"],
                "requestAccountingComplete": True,
                "usage": self.write_file(f"{kind}-{side}-usage.json", usage),
                "artifacts": artifacts,
                "review": {
                    "artifactSetSha256": ACCEPTANCE.identity(
                        {item["kind"]: item["sha256"] for item in artifacts}),
                    "reviewer": "synthetic test fixture, not a real reviewer",
                    "criteria": dict.fromkeys(ACCEPTANCE.CRITERIA, "pass"),
                    "objectives": dict.fromkeys(case["objectiveIds"], "covered"),
                },
            }
        return case

    def claim_live_for_comparator_test(self):
        # Exercise the operator-attested branch, NOT a live provider or real acceptance.
        for case in self.manifest["cases"]:
            for side in ("baseline", "candidate"):
                case[side]["evidenceKind"] = "live_provider"

    def run_record(self, side="candidate", index=0):
        return self.manifest["cases"][index][side]

    def change_usage(self, change, side="candidate", index=0):
        run = self.run_record(side, index)
        path = run["usage"]["path"]
        usage = json.loads((self.root / path).read_bytes())
        change(usage)
        run["usage"] = self.write_file(path, usage)

    def evaluate(self):
        self.manifest_path.write_text(json.dumps(self.manifest))
        return ACCEPTANCE.evaluate(self.manifest_path)

    def codes(self, result):
        return {item["code"] for case in result["cases"] for item in case["issues"]}

    def assert_invalid(self, expected):
        with self.assertRaisesRegex(ACCEPTANCE.EvidenceError, f"^{expected}$"):
            self.evaluate()

    def test_complete_submitted_evidence_can_pass_comparator_not_prove_origin(self):
        self.claim_live_for_comparator_test()
        result = self.evaluate()
        self.assertEqual(result["verdict"], "accepted")
        self.assertEqual(result["missingCaseKinds"], [])
        self.assertIn("does not independently prove", result["notes"][0])

    def test_synthetic_matrix_never_qualifies_live_acceptance(self):
        result = self.evaluate()
        self.assertEqual(result["verdict"], "incomplete")
        self.assertIn("synthetic_not_live_acceptance", self.codes(result))

    def test_missing_representative_case_is_not_a_pass(self):
        self.claim_live_for_comparator_test()
        self.manifest["cases"] = self.manifest["cases"][:-1]
        result = self.evaluate()
        self.assertEqual(result["verdict"], "incomplete")
        self.assertEqual(len(result["missingCaseKinds"]), 1)

    def test_quality_failure_cannot_be_offset_by_lower_token_usage(self):
        self.run_record()["review"]["criteria"]["preservedQualifications"] = "fail"
        self.change_usage(lambda data: data["requests"][0].update(inputTokens=1, outputTokens=5))
        result = self.evaluate()
        self.assertEqual(result["verdict"], "rejected")
        self.assertIn("quality_preservedQualifications", self.codes(result))

    def test_failed_baseline_is_not_a_valid_performance_comparison(self):
        self.change_usage(lambda data: data["operation"].update(state="failed"), side="baseline")
        self.assertEqual(self.evaluate()["verdict"], "rejected")

    def test_missing_review_criteria_remain_unobserved(self):
        self.claim_live_for_comparator_test()
        del self.run_record()["review"]["criteria"]["readability"]
        result = self.evaluate()
        self.assertEqual(result["verdict"], "incomplete")
        self.assertIn("quality_readability", self.codes(result))

    def test_omitted_and_failed_objectives_are_distinguished(self):
        review = self.run_record()["review"]["objectives"]
        del review["core"]
        self.assertEqual(self.evaluate()["verdict"], "incomplete")
        review["core"] = "not_covered"
        self.assertEqual(self.evaluate()["verdict"], "rejected")

    def test_extra_objective_cannot_replace_omitted_original(self):
        self.run_record()["review"]["objectives"] = {"replacement": "covered"}
        self.assert_invalid("unexpected_review_objective")

    def test_published_guide_cannot_stand_in_for_missing_cards(self):
        run = self.run_record()
        run["artifacts"] = [item for item in run["artifacts"] if item["kind"] == "guide"]
        run["review"]["artifactSetSha256"] = ACCEPTANCE.identity(
            {item["kind"]: item["sha256"] for item in run["artifacts"]})
        self.assertIn("published_artifact_missing", self.codes(self.evaluate()))

    def test_changed_artifact_bytes_invalidate_reviewed_evidence(self):
        artifact = self.run_record()["artifacts"][0]
        (self.root / artifact["path"]).write_text("different output")
        self.assert_invalid("file_hash_mismatch")

    def test_empty_artifact_cannot_be_accepted_even_with_updated_hash_and_review(self):
        run = self.run_record()
        artifact = run["artifacts"][0]
        artifact.update(self.write_file(artifact["path"], "", plain=True))
        run["review"]["artifactSetSha256"] = ACCEPTANCE.identity(
            {item["kind"]: item["sha256"] for item in run["artifacts"]})
        self.assert_invalid("evidence_file_empty")

    def test_updated_artifact_hash_without_new_review_is_rejected(self):
        run = self.run_record()
        artifact = run["artifacts"][0]
        artifact.update(self.write_file(artifact["path"], {"new": "output"}))
        self.assert_invalid("review_artifact_mismatch")

    def test_sources_configuration_heads_and_cache_state_must_match(self):
        original = copy.deepcopy(self.run_record())
        for field, value, expected in (
            ("headSha", "c" * 40, "run_head_mismatch"),
            ("sourceSetSha256", "c" * 64, "run_source_mismatch"),
            ("configurationSha256", "c" * 64, "run_configuration_mismatch"),
            ("cacheState", "warm", "cache_state_mismatch"),
        ):
            with self.subTest(field=field):
                self.manifest["cases"][0]["candidate"] = copy.deepcopy(original)
                self.run_record()[field] = value
                self.assert_invalid(expected)

    def test_unconfirmed_request_accounting_does_not_become_zero(self):
        self.run_record()["requestAccountingComplete"] = False
        result = self.evaluate()
        self.assertIsNone(result["cases"][0]["candidate"]["inputOutputTokens"])
        self.assertIn("request_accounting_unconfirmed", self.codes(result))

    def test_partial_input_usage_stays_unknown(self):
        self.change_usage(lambda data: data["requests"][0].update(inputTokens=None))
        result = self.evaluate()
        metrics = result["cases"][0]["candidate"]
        self.assertIsNone(metrics["reportedInputTokens"])
        self.assertIsNone(metrics["inputOutputTokens"])
        self.assertEqual(metrics["missingInputUsageRequests"], 1)

    def test_boolean_negative_fractional_and_oversized_tokens_are_not_measurements(self):
        for value in (True, -1, 3.5, 2 ** 64):
            with self.subTest(value=value):
                self.change_usage(lambda data: data["requests"][0].update(inputTokens=value))
                result = self.evaluate()
                self.assertIsNone(result["cases"][0]["candidate"]["inputOutputTokens"])

    def test_precomputed_summary_cannot_hide_actual_usage_regression(self):
        def change(data):
            data["requests"][0]["inputTokens"] = 1000
            data["totals"] = {"inputTokens": 0, "outputTokens": 0}
            data["wallClockMs"] = 1
        self.change_usage(change)
        result = self.evaluate()
        self.assertEqual(result["verdict"], "rejected")
        self.assertEqual(result["cases"][0]["candidate"]["inputOutputTokens"], 1020)
        self.assertEqual(result["cases"][0]["candidate"]["operationElapsedMs"], 800)

    def test_cache_and_reasoning_are_not_double_counted(self):
        self.change_usage(lambda data: data["requests"][0].update(
            cachedInputTokens=70, reasoningTokens=19))
        metrics = self.evaluate()["cases"][0]["candidate"]
        self.assertEqual(metrics["inputOutputTokens"], 100)
        self.assertEqual(metrics["cacheReadPercentOfCoveredInput"], 87.5)

    def test_impossible_usage_subset_is_invalid(self):
        self.change_usage(lambda data: data["requests"][0].update(reasoningTokens=21))
        self.assert_invalid("usage_subset_exceeds_total")

    def test_duplicate_and_cross_operation_requests_are_invalid(self):
        self.change_usage(lambda data: data["requests"].append(dict(data["requests"][0])))
        self.assert_invalid("request_identity_ambiguous")
        self.change_usage(lambda data: data["requests"].pop())
        self.change_usage(lambda data: data["requests"][0].update(operationId="other"))
        self.assert_invalid("cross_operation_request")

    def test_zero_requests_need_both_attestation_and_durable_reuse(self):
        self.change_usage(lambda data: data.update(requests=[]))
        self.run_record()["zeroRequestReuseObserved"] = True
        first = self.evaluate()
        self.assertIsNone(first["cases"][0]["candidate"]["inputOutputTokens"])
        self.assertIsNone(first["cases"][0]["candidate"]["reportedInputTokens"])
        self.assertIsNone(first["cases"][0]["candidate"]["reportedOutputTokens"])
        self.change_usage(lambda data: data.update(localReuse=[
            {"operationId": data["operationId"], "stage": "guide_authoring", "count": 1}]))
        second = self.evaluate()
        self.assertEqual(second["cases"][0]["candidate"]["inputOutputTokens"], 0)
        self.assertNotIn("zero_request_reuse_unconfirmed", self.codes(second))

    def test_duration_uses_operation_bounds_not_overlapping_requests(self):
        def change(data):
            row = data["requests"][0]
            row.update(startedAt=100, finishedAt=800, inputTokens=40, outputTokens=10)
            second = dict(row, requestId="another_request")
            data["requests"].append(second)
        self.change_usage(change)
        self.assertEqual(self.evaluate()["cases"][0]["candidate"]["operationElapsedMs"], 800)

    def test_missing_and_reversed_operation_duration_cannot_pass(self):
        self.change_usage(lambda data: data["operation"].update(finishedAt=None))
        self.assertIn("operation_duration_unknown", self.codes(self.evaluate()))
        self.change_usage(lambda data: data["operation"].update(finishedAt=99))
        self.assertIn("operation_duration_unknown", self.codes(self.evaluate()))

    def test_one_case_cost_or_latency_regression_cannot_be_averaged_away(self):
        for metric in ("tokens", "duration"):
            with self.subTest(metric=metric):
                if metric == "tokens":
                    self.change_usage(lambda data: data["requests"][0].update(inputTokens=101))
                else:
                    self.change_usage(lambda data: data["requests"][0].update(inputTokens=80))
                    self.change_usage(lambda data: data["operation"].update(finishedAt=1101))
                self.assertEqual(self.evaluate()["verdict"], "rejected")

    def test_limits_are_explicit_not_invented_by_comparator(self):
        del self.manifest["limits"]
        self.assert_invalid("explicit_comparison_limits_required")

    def test_unstarted_unknown_request_cannot_be_declared_terminal(self):
        self.change_usage(lambda data: data["requests"][0].update(status="invented_success"))
        self.assertIn("request_not_terminal", self.codes(self.evaluate()))

    def test_reused_usage_snapshot_cannot_fake_multiple_representative_cases(self):
        self.manifest["cases"].append(copy.deepcopy(self.manifest["cases"][0]))
        self.manifest["cases"][-1]["id"] = "second_assessment"
        self.assert_invalid("usage_snapshot_reused_across_cases")

    def test_duplicate_json_keys_and_nonfinite_values_are_invalid(self):
        for raw, expected in ((b'{"a":1,"a":2}', "duplicate_json_key"),
                              (b'{"a":NaN}', "nonfinite_json_number"),
                              (b'{"a":1e9999}', "nonfinite_json_number")):
            with self.subTest(raw=raw):
                with self.assertRaisesRegex(ACCEPTANCE.EvidenceError, expected):
                    ACCEPTANCE.parse_json(raw)

    def test_absolute_and_symlink_escape_paths_are_rejected(self):
        reference = self.run_record()["usage"]
        original = reference["path"]
        reference["path"] = str(self.root / original)
        self.assert_invalid("relative_file_required")
        outside = self.root.parent / (self.root.name + "-outside.json")
        outside.write_text("{}")
        self.addCleanup(outside.unlink)
        link = self.root / "outside-link.json"
        link.symlink_to(outside)
        reference.update(path=link.name, sha256=hashlib.sha256(b"{}").hexdigest())
        self.assert_invalid("file_outside_evidence_root")

    def test_unknown_versions_and_duplicate_case_ids_are_rejected(self):
        self.manifest["schemaVersion"] = True
        self.assert_invalid("manifest_version_invalid")
        self.manifest["schemaVersion"] = 1
        self.manifest["cases"][1]["id"] = self.manifest["cases"][0]["id"]
        self.assert_invalid("duplicate_case_identity")

    def test_read_only_execution_preserves_all_evidence_bytes(self):
        self.evaluate()
        before = {path.name: path.read_bytes() for path in self.root.iterdir()}
        ACCEPTANCE.evaluate(self.manifest_path)
        self.assertEqual(before, {path.name: path.read_bytes() for path in self.root.iterdir()})

    def test_cli_exit_statuses_and_content_free_errors(self):
        def invoke():
            self.manifest_path.write_text(json.dumps(self.manifest))
            return subprocess.run([sys.executable, str(SCRIPT), str(self.manifest_path)],
                                  text=True, capture_output=True, cwd="/", check=False)
        self.assertEqual(invoke().returncode, 2)
        self.claim_live_for_comparator_test()
        passed = invoke()
        self.assertEqual(passed.returncode, 0, passed.stdout + passed.stderr)
        self.run_record()["review"]["criteria"]["factualSupport"] = "fail"
        self.assertEqual(invoke().returncode, 1)
        self.run_record()["usage"]["path"] = "SECRET-PATIENT-TEXT-file-does-not-exist"
        invalid = invoke()
        self.assertEqual(invalid.returncode, 2)
        self.assertNotIn("SECRET-PATIENT", invalid.stdout + invalid.stderr)
        self.assertNotIn(str(self.root), invalid.stdout + invalid.stderr)
        self.assertEqual(json.loads(invalid.stdout)["verdict"], "invalid")


    def test_answer_key_failure_cannot_be_hidden_by_token_savings(self):
        self.run_record()["review"]["criteria"]["answerKeyCorrectness"] = "fail"
        result = self.evaluate()
        self.assertEqual(result["verdict"], "rejected")
        self.assertIn("quality_answerKeyCorrectness", self.codes(result))

    def test_first_usable_latency_is_independent_of_provider_and_completion_times(self):
        result = self.evaluate()
        metrics = result["cases"][0]["candidate"]
        self.assertEqual(metrics["firstUsableArtifactMs"], 400)
        self.assertEqual(metrics["operationElapsedMs"], 800)
        self.assertEqual(result["cases"][0]["ratios"]["firstUsableArtifactMs"], 0.8)

    def test_missing_availability_is_unknown_not_inferred_from_provider(self):
        self.claim_live_for_comparator_test()
        del self.run_record()["artifacts"][0]["usableAt"]
        result = self.evaluate()
        self.assertEqual(result["verdict"], "incomplete")
        self.assertIsNone(result["cases"][0]["candidate"]["firstUsableArtifactMs"])

    def test_impossible_availability_and_booleans_do_not_become_latency(self):
        for value in (99, 901):
            with self.subTest(value=value):
                self.run_record()["artifacts"][0]["usableAt"] = value
                self.assert_invalid("artifact_availability_outside_run")
        self.run_record()["artifacts"][0]["usableAt"] = True
        self.assertIn("first_usable_artifact_unknown", self.codes(self.evaluate()))

    def test_lower_completion_time_does_not_hide_slower_first_artifact(self):
        self.claim_live_for_comparator_test()
        for artifact in self.run_record()["artifacts"]:
            artifact["usableAt"] = 800
        result = self.evaluate()
        self.assertEqual(result["verdict"], "rejected")
        self.assertIn("firstUsableArtifactMs_limit_exceeded", self.codes(result))

    def test_fresh_and_resume_evidence_are_required_and_never_paired_together(self):
        self.claim_live_for_comparator_test()
        for case in self.manifest["cases"]:
            case["regime"] = "fresh"
            for side in ("baseline", "candidate"):
                case[side]["generationRegime"] = "fresh"
        result = self.evaluate()
        self.assertEqual(result["verdict"], "incomplete")
        self.assertEqual(result["missingGenerationRegimes"], ["resume"])
        self.run_record()["generationRegime"] = "resume"
        self.assert_invalid("generation_regime_mismatch")


if __name__ == "__main__":
    unittest.main()
