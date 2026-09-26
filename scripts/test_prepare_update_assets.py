import tempfile
import unittest
import json
from pathlib import Path

from prepare_update_assets import GROUPS, collect, prepare


class UpdateAssetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "staging"
        for group, suffixes in GROUPS.items():
            directory = self.source / group
            directory.mkdir(parents=True)
            for suffix in suffixes:
                content = b"test-signature" if suffix.endswith(".sig") else b"test-installer"
                (directory / ("bundle_0.1.0-alpha.4" + suffix)).write_bytes(content)

    def test_manifest_points_at_exact_published_files(self):
        output = self.root / "release"
        manifest = prepare(self.source, output, "0.1.0-alpha.4")
        self.assertEqual(len(list(output.iterdir())), 13)
        self.assertEqual(manifest["version"], "0.1.0-alpha.4")
        for platform in manifest["platforms"].values():
            self.assertEqual(platform["signature"], "test-signature")
            self.assertIn("/releases/download/v0.1.0-alpha.4/", platform["url"])
            self.assertTrue((output / platform["url"].rsplit("/", 1)[1]).exists())
        self.assertEqual(
            set(manifest["platforms"]),
            {"darwin-x86_64", "windows-x86_64"},
        )
        for kind in ("deb", "rpm"):
            feed = json.loads((output / f"latest-{kind}.json").read_text())
            package = feed["platforms"]["linux-x86_64"]
            self.assertEqual(feed["version"], "0.1.0-alpha.4")
            self.assertTrue(package["url"].endswith(f".{kind}"))
            self.assertEqual(package["signature"], "test-signature")

    def test_missing_signature_rejects_the_staging_build(self):
        (self.source / "preview-linux-x64-deb" / "bundle_0.1.0-alpha.4.deb.sig").unlink()
        with self.assertRaisesRegex(ValueError, "expected 2 files"):
            collect(self.source)

    def test_signature_name_must_match_the_installer(self):
        group = self.source / "preview-linux-x64-rpm"
        (group / "bundle_0.1.0-alpha.4.rpm.sig").rename(group / "other_0.1.0-alpha.4.rpm.sig")
        with self.assertRaisesRegex(ValueError, "signature does not match"):
            collect(self.source)


if __name__ == "__main__":
    unittest.main()
