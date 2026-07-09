import importlib.util
import os
import shutil
import tempfile
import unittest
from pathlib import Path

# codex-instruct.py uses a hyphen, so load it manually rather than renaming it.
spec = importlib.util.spec_from_file_location(
    "codex_instruct",
    Path(__file__).resolve().parent.parent / "codex-instruct.py",
)
codex_instruct = importlib.util.module_from_spec(spec)
spec.loader.exec_module(codex_instruct)


class TestFindCodexDirs(unittest.TestCase):
    def setUp(self) -> None:
        self.original = os.environ.get("CODEX_HOME", "")
        self.tmpdir = tempfile.mkdtemp()
        self.addCleanup(self._restore_env)
        self.addCleanup(lambda: shutil.rmtree(self.tmpdir, ignore_errors=True))

    def _restore_env(self) -> None:
        if self.original:
            os.environ["CODEX_HOME"] = self.original
        else:
            os.environ.pop("CODEX_HOME", None)

    def test_finds_codex_home(self) -> None:
        codex_root = Path(self.tmpdir) / ".codex"
        codex_root.mkdir()
        (codex_root / "config.toml").write_text("[core]", encoding="utf-8")
        os.environ["CODEX_HOME"] = str(codex_root)
        found = codex_instruct.find_codex_dirs()
        self.assertIn(str(codex_root.resolve()), found)

    def test_returns_empty_when_nothing_found(self) -> None:
        os.environ.pop("CODEX_HOME", None)
        # Point USERPROFILE at an empty temp dir so no real .codex is found.
        original_userprofile = os.environ.get("USERPROFILE")
        os.environ["USERPROFILE"] = self.tmpdir
        self.addCleanup(
            lambda: (
                os.environ.__setitem__("USERPROFILE", original_userprofile)
                if original_userprofile
                else os.environ.pop("USERPROFILE", None)
            )
        )
        found = codex_instruct.find_codex_dirs()
        self.assertEqual(found, [])


class TestBackupConfig(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.mkdtemp()
        self.addCleanup(lambda: shutil.rmtree(self.tmpdir, ignore_errors=True))

    def test_creates_timestamped_backup(self) -> None:
        config = Path(self.tmpdir) / "config.toml"
        original = "model = 'gpt-5.5'"
        config.write_text(original, encoding="utf-8")
        backup = codex_instruct.backup_config(config)
        self.assertTrue(backup.exists())
        self.assertNotEqual(backup.name, config.name)
        self.assertTrue(backup.name.startswith("config.toml.bak_"))
        self.assertEqual(backup.read_text(encoding="utf-8"), original)


class TestEnsureModelInstructions(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.mkdtemp()
        self.addCleanup(lambda: shutil.rmtree(self.tmpdir, ignore_errors=True))

    def test_inserts_after_model_line(self) -> None:
        config = Path(self.tmpdir) / "config.toml"
        config.write_text("model = 'gpt-5.5'\n", encoding="utf-8")
        changed = codex_instruct.ensure_model_instructions(config, "gpt5.5-unrestricted.md")
        self.assertTrue(changed)
        text = config.read_text(encoding="utf-8")
        self.assertIn('model_instructions_file = "./gpt5.5-unrestricted.md"', text)

    def test_updates_existing_line(self) -> None:
        config = Path(self.tmpdir) / "config.toml"
        config.write_text(
            "model = 'gpt-5.5'\nmodel_instructions_file = \"./old.md\"\n",
            encoding="utf-8",
        )
        changed = codex_instruct.ensure_model_instructions(config, "gpt5.5-unrestricted.md")
        self.assertTrue(changed)
        text = config.read_text(encoding="utf-8")
        self.assertIn('model_instructions_file = "./gpt5.5-unrestricted.md"', text)
        self.assertNotIn("./old.md", text)

    def test_no_change_when_value_unchanged(self) -> None:
        config = Path(self.tmpdir) / "config.toml"
        config.write_text(
            "model = 'gpt-5.5'\nmodel_instructions_file = \"./gpt5.5-unrestricted.md\"\n",
            encoding="utf-8",
        )
        changed = codex_instruct.ensure_model_instructions(config, "gpt5.5-unrestricted.md")
        self.assertFalse(changed)


if __name__ == "__main__":
    unittest.main()
