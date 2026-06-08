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
BASH = "/usr/bin/bash"


class BuildResourcepackTest(unittest.TestCase):
    def test_builds_full_pack_manifest_and_sha1(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            assets = root / "assets"
            out = root / "out"
            self._write(assets / "minecraft" / "textures" / "block" / "copper_ore.png", b"mineral")
            self._write(assets / "bong" / "geo" / "rat.geo.JSON", b'{"format_version":"1.12.0"}')
            self._write(assets / "bong" / "models" / "item" / "bone_dagger" / "bone_dagger.OBJ", b"obj")
            self._write(assets / "bong" / "models" / "item" / "bone_dagger" / "bone_dagger.MTL", b"mtl")
            self._write(assets / "bong" / "textures" / "entity" / "rat.PNG", b"entity")
            self._write(assets / "bong" / "particles" / "ash.json", b'{"particle_effect":{}}')
            self._write(assets / "bong" / "textures" / "particle" / "ash.png", b"vfx")
            self._write(assets / "bong-client" / "textures" / "hud" / "effects" / "bleeding.png", b"hud")
            self._write(assets / "bong-client" / "textures" / "gui" / "items" / "huge_icon.png", b"ui")
            self._write(assets / "bong" / "audio_recipes" / "wind.json", b'{"id":"wind"}')
            self._write(assets / "bong" / "atmosphere" / "wind.ogg", b"ogg")

            env = self._env(assets, out, version="test")
            subprocess.run([BASH, str(SCRIPT)], check=True, cwd=REPO_ROOT, env=env)

            pack = out / "bong-full-test.zip"
            manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
            sha1 = hashlib.sha1(pack.read_bytes(), usedforsecurity=False).hexdigest()

            self.assertEqual(
                "bong-full",
                manifest["name"],
                f"expected bong-full because manifest identifies the generated full pack, actual {manifest['name']}",
            )
            self.assertEqual(
                "test",
                manifest["version"],
                f"expected test because BONG_RESOURCEPACK_VERSION overrides output version, actual {manifest['version']}",
            )
            self.assertEqual(
                "bong-full-test.zip",
                manifest["file"],
                f"expected bong-full-test.zip because version test controls pack filename, actual {manifest['file']}",
            )
            self.assertEqual(
                sha1,
                manifest["sha1"],
                f"expected manifest sha1 {sha1} because it must match generated zip bytes, actual {manifest['sha1']}",
            )
            self.assertEqual(
                sha1,
                (out / "bong-full-test.zip.sha1").read_text(encoding="utf-8").strip(),
                f"expected sha1 sidecar {sha1} because server config consumes the same hash, actual {(out / 'bong-full-test.zip.sha1').read_text(encoding='utf-8').strip()}",
            )
            self.assertEqual(
                pack.stat().st_size,
                manifest["size"],
                f"expected manifest size {pack.stat().st_size} because client cache validation uses zip byte size, actual {manifest['size']}",
            )
            self.assertFalse(
                manifest["force_accept_default"],
                f"expected force_accept_default false because resource pack decline degrades instead of kicking, actual {manifest['force_accept_default']}",
            )
            self.assertEqual(
                {"mineral", "entity-model", "vfx", "audio"},
                {entry["id"] for entry in manifest["packs"]},
                f"expected four P0 subpacks because P0 covers mineral/entity-model/vfx/audio, actual {manifest['packs']}",
            )

            counts = {entry["id"]: entry["file_count"] for entry in manifest["packs"]}
            self.assertEqual(1, counts["mineral"], f"expected one mineral fixture, actual {counts['mineral']}")
            self.assertEqual(4, counts["entity-model"], f"expected geo/obj/mtl/entity texture fixtures, actual {counts['entity-model']}")
            self.assertEqual(3, counts["vfx"], f"expected particle json/texture/hud effect fixtures, actual {counts['vfx']}")
            self.assertEqual(2, counts["audio"], f"expected audio recipe plus ogg fixtures, actual {counts['audio']}")

            with zipfile.ZipFile(pack) as zf:
                names = set(zf.namelist())
            self.assertIn("pack.mcmeta", names, f"expected pack.mcmeta because Minecraft requires pack metadata, actual names={sorted(names)}")
            self.assertIn("assets/minecraft/textures/block/copper_ore.png", names, "expected mineral texture in zip because P0 includes mineral assets")
            self.assertIn("assets/bong/geo/rat.geo.JSON", names, "expected uppercase .JSON accepted because filtering is case-insensitive")
            self.assertIn("assets/bong/models/item/bone_dagger/bone_dagger.OBJ", names, "expected uppercase .OBJ accepted because model assets include OBJ runtime resources")
            self.assertIn("assets/bong/models/item/bone_dagger/bone_dagger.MTL", names, "expected uppercase .MTL accepted because OBJ materials must travel with models")
            self.assertIn("assets/bong/textures/entity/rat.PNG", names, "expected uppercase .PNG accepted because image suffix matching is case-insensitive")
            self.assertIn("assets/bong/particles/ash.json", names, "expected particle json in zip because P0 includes VFX definitions")
            self.assertIn("assets/bong/textures/particle/ash.png", names, "expected particle texture in zip because P0 includes VFX textures")
            self.assertIn("assets/bong-client/textures/hud/effects/bleeding.png", names, "expected HUD effect texture in zip because status-effect VFX assets are included")
            self.assertNotIn("assets/bong-client/textures/gui/items/huge_icon.png", names, "expected GUI item icon excluded because P0 avoids huge non-resourcepack UI icon payload")

    def test_empty_assets_tree_builds_metadata_only_pack(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            assets = root / "assets"
            out = root / "out"
            assets.mkdir()

            subprocess.run([BASH, str(SCRIPT)], check=True, cwd=REPO_ROOT, env=self._env(assets, out, version="empty"))

            pack = out / "bong-full-empty.zip"
            manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
            with zipfile.ZipFile(pack) as zf:
                names = set(zf.namelist())
            self.assertEqual({"pack.mcmeta"}, names, f"expected only pack.mcmeta for empty assets tree, actual {sorted(names)}")
            self.assertTrue(all(entry["file_count"] == 0 for entry in manifest["packs"]), f"expected all file counts zero for empty assets tree, actual {manifest['packs']}")

    def test_filter_excludes_unsupported_suffix_and_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            assets = root / "assets"
            out = root / "out"
            self._write(assets / "bong" / "textures" / "particle" / "ok.png", b"ok")
            self._write(assets / "bong" / "textures" / "particle" / "source.psd", b"psd")
            self._write(assets / "bong-client" / "textures" / "gui" / "items" / "skip.png", b"skip")

            subprocess.run([BASH, str(SCRIPT)], check=True, cwd=REPO_ROOT, env=self._env(assets, out, version="filter"))

            with zipfile.ZipFile(out / "bong-full-filter.zip") as zf:
                names = set(zf.namelist())
            self.assertIn("assets/bong/textures/particle/ok.png", names, "expected included particle png because prefix and suffix are both allowed")
            self.assertNotIn("assets/bong/textures/particle/source.psd", names, "expected psd excluded because suffix is unsupported runtime payload")
            self.assertNotIn("assets/bong-client/textures/gui/items/skip.png", names, "expected gui item icon excluded because prefix is outside P0 resourcepack whitelist")

    def test_rejects_missing_assets_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            env = self._env(Path(tmp) / "missing", Path(tmp) / "out")
            proc = subprocess.run(
                [BASH, str(SCRIPT)],
                cwd=REPO_ROOT,
                env=env,
                text=True,
                stderr=subprocess.PIPE,
                check=False,
            )

            self.assertNotEqual(0, proc.returncode, f"expected non-zero because assets root is missing, actual {proc.returncode}")
            self.assertIn("missing resource asset source directory", proc.stderr, f"expected missing root error hint, actual {proc.stderr}")

    @staticmethod
    def _env(assets: Path, out: Path, version: str = "test") -> dict[str, str]:
        env = os.environ.copy()
        env.update(
            {
                "BONG_RESOURCEPACK_ASSETS_ROOT": str(assets),
                "BONG_RESOURCEPACK_OUT_DIR": str(out),
                "BONG_RESOURCEPACK_VERSION": version,
                "BONG_RESOURCEPACK_BUILD_EPOCH": "202606080000.00",
            }
        )
        return env

    @staticmethod
    def _write(path: Path, content: bytes) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)


if __name__ == "__main__":
    unittest.main()
