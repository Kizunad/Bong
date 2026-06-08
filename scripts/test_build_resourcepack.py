from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "build-resourcepack.sh"


class BuildResourcepackTest(unittest.TestCase):
    def test_builds_full_pack_manifest_and_sha1(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            assets = root / "assets"
            out = root / "out"
            self._write(assets / "minecraft" / "textures" / "block" / "copper_ore.png", b"mineral")
            self._write(assets / "bong" / "geo" / "rat.geo.json", b'{"format_version":"1.12.0"}')
            self._write(assets / "bong" / "models" / "item" / "bone_dagger" / "bone_dagger.obj", b"obj")
            self._write(assets / "bong" / "models" / "item" / "bone_dagger" / "bone_dagger.mtl", b"mtl")
            self._write(assets / "bong" / "textures" / "entity" / "rat.png", b"entity")
            self._write(assets / "bong" / "particles" / "ash.json", b'{"particle_effect":{}}')
            self._write(assets / "bong" / "textures" / "particle" / "ash.png", b"vfx")
            self._write(assets / "bong-client" / "textures" / "hud" / "effects" / "bleeding.png", b"hud")
            self._write(assets / "bong-client" / "textures" / "gui" / "items" / "huge_icon.png", b"ui")
            self._write(assets / "bong" / "audio_recipes" / "wind.json", b'{"id":"wind"}')
            self._write(assets / "bong" / "atmosphere" / "wind.ogg", b"ogg")

            env = os.environ.copy()
            env.update(
                {
                    "BONG_RESOURCEPACK_ASSETS_ROOT": str(assets),
                    "BONG_RESOURCEPACK_OUT_DIR": str(out),
                    "BONG_RESOURCEPACK_VERSION": "test",
                    "BONG_RESOURCEPACK_BUILD_EPOCH": "202606080000.00",
                }
            )
            subprocess.run(["bash", str(SCRIPT)], check=True, cwd=REPO_ROOT, env=env)

            pack = out / "bong-full-test.zip"
            manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
            sha1 = hashlib.sha1(pack.read_bytes()).hexdigest()

            self.assertEqual("bong-full", manifest["name"])
            self.assertEqual("test", manifest["version"])
            self.assertEqual("bong-full-test.zip", manifest["file"])
            self.assertEqual(sha1, manifest["sha1"])
            self.assertEqual(sha1, (out / "bong-full-test.zip.sha1").read_text(encoding="utf-8").strip())
            self.assertEqual(pack.stat().st_size, manifest["size"])
            self.assertFalse(manifest["force_accept_default"])
            self.assertEqual(
                {"mineral", "entity-model", "vfx", "audio"},
                {entry["id"] for entry in manifest["packs"]},
            )

            with zipfile.ZipFile(pack) as zf:
                names = set(zf.namelist())
            self.assertIn("pack.mcmeta", names)
            self.assertIn("assets/minecraft/textures/block/copper_ore.png", names)
            self.assertIn("assets/bong/geo/rat.geo.json", names)
            self.assertIn("assets/bong/models/item/bone_dagger/bone_dagger.obj", names)
            self.assertIn("assets/bong/models/item/bone_dagger/bone_dagger.mtl", names)
            self.assertIn("assets/bong/textures/entity/rat.png", names)
            self.assertIn("assets/bong/particles/ash.json", names)
            self.assertIn("assets/bong/textures/particle/ash.png", names)
            self.assertIn("assets/bong-client/textures/hud/effects/bleeding.png", names)
            self.assertNotIn("assets/bong-client/textures/gui/items/huge_icon.png", names)

    def test_rejects_missing_assets_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            env = os.environ.copy()
            env.update(
                {
                    "BONG_RESOURCEPACK_ASSETS_ROOT": str(Path(tmp) / "missing"),
                    "BONG_RESOURCEPACK_OUT_DIR": str(Path(tmp) / "out"),
                }
            )
            proc = subprocess.run(
                ["bash", str(SCRIPT)],
                cwd=REPO_ROOT,
                env=env,
                text=True,
                stderr=subprocess.PIPE,
            )

            self.assertNotEqual(0, proc.returncode)
            self.assertIn("missing resource asset source directory", proc.stderr)

    @staticmethod
    def _write(path: Path, content: bytes) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)


if __name__ == "__main__":
    unittest.main()
