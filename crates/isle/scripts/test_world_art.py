"""Offline tests for the PixelLab art library and its spending/retry guards. Never calls PixelLab."""

import base64
import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

import world_art as art

COLORS = {"water": (20, 90, 110), "grass": (80, 150, 70), "dirt": (190, 160, 110), "sand": (240, 220, 160),
          "rock": (80, 80, 90), "floor": (160, 150, 130), "wall": (35, 20, 15)}
RIM = (250, 230, 190)


def encode(image):
    data = io.BytesIO()
    image.save(data, format="PNG")
    return base64.b64encode(data.getvalue()).decode()


def tileset(sides):
    """A synthetic tileset: each tile quarter is the flat color of its corner's terrain."""
    tiles = []
    for mask in range(16):
        corners = {k: ("upper" if mask & (8 >> i) else "lower") for i, k in enumerate(art.CORNERS)}
        image = Image.new("RGBA", (32, 32))
        for i, k in enumerate(art.CORNERS):
            color = COLORS[sides[corners[k]]]
            image.paste((*color, 255), ((i % 2) * 16, (i // 2) * 16, (i % 2) * 16 + 16, (i // 2) * 16 + 16))
        if 0 < mask < 15:
            image.paste((*RIM, 255), (15, 0, 17, 32))  # A transition band, like the shore's sand rim.
        tiles.append({"corners": corners, "image": {"base64": encode(image)}})
    return {"tileset": {"tiles": tiles}, "usage": {"type": "generations", "generations": 1}}


def picture(size, filled):
    image = Image.new("RGBA", (size, size))
    for x0, y0 in filled:
        image.paste((200, 60, 60, 255), (x0 + 8, y0 + 8, x0 + 24, y0 + 24))
    return {"image": {"base64": encode(image)}, "usage": {"type": "generations", "generations": 1}}


class WorldArt(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="isle-art-test-")
        self.addCleanup(self.temp.cleanup)
        root = pathlib.Path(self.temp.name)
        for name, value in [("RAW", root / "raw"), ("ASSETS", root / "assets")]:
            change = patch.object(art, name, value)
            change.start()
            self.addCleanup(change.stop)
        art.RAW.mkdir()
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)

    def store(self, name, result):
        folder = art.RAW / name
        folder.mkdir(parents=True)
        data = json.dumps(result).encode()
        (folder / "original.json").write_bytes(data)
        (folder / "receipt.json").write_text(json.dumps({"original_sha256": art.sha(data), "usage": result["usage"]}))

    def full_library(self):
        for name, spec in art.TERRAIN.items():
            self.store(f"terrain-{name}", tileset(spec["sides"]))
        self.store("objects", picture(128, [(x * 32, y * 32) for y in range(4) for x in range(4)]))
        for name in art.SINGLES:
            self.store(f"object-{name}", picture(32, [(0, 0)]))

    def test_process_packs_tiles_by_corner_mask_with_verifiable_provenance(self):
        self.full_library()
        art.process()
        manifest = json.loads((art.ASSETS / "manifest.json").read_text())
        for name, spec in art.TERRAIN.items():
            info = manifest["terrain"][name]
            file = art.ASSETS / info["file"]
            self.assertEqual(info["image_sha256"], art.sha(file.read_bytes()))
            self.assertEqual((info["a"], info["b"]), (spec["sides"]["lower"], spec["sides"]["upper"]))
            with Image.open(file) as sheet:
                sheet = sheet.convert("RGBA")
                self.assertEqual(sheet.size, (128, 128))
                if any(key[0] == name for key in art.MATCH):
                    continue  # Recolored; covered below.
                for mask in range(16):
                    for i in range(4):
                        x = (mask % 4) * 32 + (i % 2) * 16 + 8
                        y = (mask // 4) * 32 + (i // 2) * 16 + 8
                        terrain = info["b"] if mask & (8 >> i) else info["a"]
                        self.assertEqual(sheet.getpixel((x, y)), (*COLORS[terrain], 255))
        self.assertEqual(manifest["objects"]["order"], art.OBJECTS)
        with Image.open(art.ASSETS / "objects.png") as objects:
            self.assertEqual({a for a in objects.getchannel("A").get_flattened_data()}, {0, 255})

    def test_shared_terrain_is_recolored_to_the_reference_set(self):
        self.full_library()
        art.process()
        with Image.open(art.ASSETS / "terrain_path.png") as path, Image.open(art.ASSETS / "terrain_shore.png") as shore:
            # Path's all-grass tile (mask 15) now uses the shore's grass.
            self.assertEqual(path.convert("RGBA").getpixel((3 * 32 + 8, 3 * 32 + 8)),
                             shore.convert("RGBA").getpixel((3 * 32 + 8, 3 * 32 + 8)))
            self.assertEqual(path.convert("RGBA").getpixel((8, 8)), (*COLORS["dirt"], 255))

    def test_modified_original_is_rejected(self):
        self.full_library()
        (art.RAW / "terrain-shore" / "original.json").write_text("{}")
        with self.assertRaisesRegex(RuntimeError, "receipt"):
            art.process()

    def test_objects_crossing_their_cells_are_rejected(self):
        self.full_library()
        crossing = picture(128, [])
        image = Image.new("RGBA", (128, 128))
        image.paste((200, 60, 60, 255), (20, 20, 44, 30))
        crossing["image"]["base64"] = encode(image)
        for file in (art.RAW / "objects").iterdir():
            file.unlink()
        (art.RAW / "objects").rmdir()
        self.store("objects", crossing)
        with self.assertRaisesRegex(RuntimeError, "crosses"):
            art.process()

    def test_library_entries_are_never_generated_twice(self):
        self.store("terrain-shore", tileset(art.TERRAIN["shore"]["sides"]))
        with patch.object(art, "api", return_value={"subscription": {"generations": 39}}) as api:
            art.generate(["terrain-shore"], 5)
        self.assertEqual([c.args for c in api.call_args_list], [("/balance",)])

    def test_uncertain_request_and_budget_stop_before_any_post(self):
        for budget, pending in [(0, False), (5, True)]:
            with self.subTest(pending=pending):
                if pending:
                    (art.RAW / "objects").mkdir(exist_ok=True)
                    (art.RAW / "objects" / "pending.json").write_text(json.dumps({"state": "submitting"}))
                with patch.object(art, "api", return_value={"subscription": {"generations": 40}}) as api:
                    if pending:
                        with self.assertRaisesRegex(RuntimeError, "may have been charged"):
                            art.generate(["objects"], budget)
                    else:
                        art.generate(["objects"], budget)
                self.assertEqual([c.args for c in api.call_args_list], [("/balance",)])

    def test_slow_job_is_left_pending_then_resumed_without_resubmitting(self):
        balance = {"subscription": {"generations": 5}}
        submitted = {"background_job_id": "job-1", "tileset_id": "set-1"}
        with patch.object(art, "api", side_effect=[balance, submitted, {"status": "processing"}]):
            art.generate(["terrain-shore"], 5, wait=0)
        pending = json.loads((art.RAW / "terrain-shore" / "pending.json").read_text())
        self.assertEqual(pending["background_job_id"], "job-1")
        finished = tileset(art.TERRAIN["shore"]["sides"])
        responses = [balance, {"status": "completed", "usage": {"type": "generations", "generations": 1}},
                     finished, {"subscription": {"generations": 4}}]
        with patch.object(art, "api", side_effect=responses) as api:
            art.generate(["terrain-shore"], 5)
        paths = [c.args[0] for c in api.call_args_list]
        self.assertNotIn("/create-tileset", paths)
        self.assertEqual(paths[1:3], ["/background-jobs/job-1", "/tilesets/set-1"])
        receipt = json.loads((art.RAW / "terrain-shore" / "receipt.json").read_text())
        self.assertEqual(receipt["usage"]["generations"], 1)
        self.assertNotIn("Authorization", json.dumps(receipt))
        self.assertFalse((art.RAW / "terrain-shore" / "pending.json").exists())

    def test_timeout_leaves_guard_so_rerun_cannot_duplicate_a_charge(self):
        balance = {"subscription": {"generations": 40}}
        with patch.object(art, "api", side_effect=[balance, TimeoutError("uncertain")]):
            with self.assertRaises(TimeoutError):
                art.generate(["objects"], 5)
        with patch.object(art, "api", return_value=balance) as api:
            with self.assertRaisesRegex(RuntimeError, "may have been charged"):
                art.generate(["objects"], 5)
        self.assertEqual(api.call_count, 1)

    def test_immediate_image_records_actual_usage_and_the_returned_bytes(self):
        result = picture(32, [(0, 0)])
        with patch.object(art, "api", side_effect=[
                {"subscription": {"generations": 40}}, result, {"subscription": {"generations": 39}}]):
            art.generate(["object-coins"], 5)
        stored = (art.RAW / "object-coins" / "original.json").read_bytes()
        self.assertEqual(json.loads(stored), result)
        receipt = json.loads((art.RAW / "object-coins" / "receipt.json").read_text())
        self.assertEqual(receipt["original_sha256"], art.sha(stored))


if __name__ == "__main__":
    unittest.main()
