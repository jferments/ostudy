from __future__ import annotations

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class MediaCspContractTests(unittest.TestCase):
    def test_managed_media_protocol_is_explicitly_allowed(self) -> None:
        config = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text())
        csp = config["app"]["security"]["csp"]
        self.assertIn(
            "media-src 'self' ostudy-asset: http://ostudy-asset.localhost https://ostudy-asset.localhost;",
            csp,
        )
        self.assertNotIn("media-src *", csp)
        self.assertNotIn("media-src https:", csp)

    def test_media_assets_remain_inside_managed_asset_scope(self) -> None:
        config = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text())
        scope = config["app"]["security"]["assetProtocol"]["scope"]
        self.assertIn("$APPDATA/assets/images/**/*", scope)
        source = (ROOT / "src" / "platform" / "assets.ts").read_text()
        self.assertIn('convertFileSrc(assetId, "ostudy-asset")', source)


if __name__ == "__main__":
    unittest.main()
