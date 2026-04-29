import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY_SCRIPT = ROOT / "scripts" / "5090jepa" / "verify_phase0.py"
GENERATOR_SCRIPT = ROOT / "scripts" / "5090jepa" / "generate_gridworld_corpus.py"
GRIDWORLD_CORPUS = ROOT / "configs" / "dynamicjepa" / "verification" / "gridworld_5x5_corpus.jsonl"


def load_phase0_verifier():
    spec = importlib.util.spec_from_file_location("verify_5090jepa_phase0", VERIFY_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class Phase0FixtureTest(unittest.TestCase):
    def test_phase0_verifier_reads_actual_fixture_files(self):
        verifier = load_phase0_verifier()
        evidence_path = ROOT / "tmp" / "5090jepa_test_phase0" / "phase0_fixture_verification.json"
        evidence_path.parent.mkdir(parents=True, exist_ok=True)

        evidence = verifier.verify_phase0(evidence_path=evidence_path)

        self.assertEqual(evidence["status"], "ok")
        self.assertEqual(evidence["source_of_truth"]["primary"], "configs/dynamicjepa files on disk")
        self.assertEqual(evidence["decoded_records"]["fixtures"]["counter_world"]["happy_path_events"], 3)
        self.assertEqual(evidence["decoded_records"]["fixtures"]["gridworld_5x5"]["corpus_events"], 8000)
        self.assertEqual(
            evidence["decoded_records"]["expected_count_contracts"]["counter_world_final_counts_from_protocol"]["dj_instrument_readings"],
            6,
        )
        self.assertEqual(
            evidence["decoded_records"]["expected_count_contracts"]["gridworld_5x5_final_counts_from_protocol"]["dj_instrument_readings"],
            24000,
        )
        self.assertTrue(evidence_path.exists())

    def test_gridworld_generator_reproduces_committed_corpus(self):
        verifier = load_phase0_verifier()
        generated = ROOT / "tmp" / "5090jepa_test_phase0" / "gridworld_5x5_corpus.jsonl"
        generated.parent.mkdir(parents=True, exist_ok=True)

        subprocess.run(
            [sys.executable, str(GENERATOR_SCRIPT), str(generated)],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )

        self.assertEqual(generated.read_bytes(), GRIDWORLD_CORPUS.read_bytes())
        self.assertEqual(verifier.sha256_file(generated), verifier.sha256_file(GRIDWORLD_CORPUS))
        self.assertEqual(len(generated.read_bytes().splitlines()), 8000)


if __name__ == "__main__":
    unittest.main()
