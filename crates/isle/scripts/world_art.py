"""PixelLab art library for Isle, never called by the game.

Run from the repository root with mise (PIXELLAB_SECRET stays in .env.local) and Pillow installed:
    python world_art.py status
    python world_art.py generate terrain-shore objects --budget 5
    python world_art.py process
    python world_art.py contact

Each library entry is generated at most once. Originals and receipts live under ignored target/; the checked-in
sheets and manifest need no credentials. A run spends at most --budget generations, so the trial's daily slow
allowance can grow the library over several days. A job still running when a run ends is resumed, not resubmitted.
"""

import argparse
import base64
import hashlib
import io
import json
import os
import pathlib
import time
import urllib.error
import urllib.request
from collections import Counter

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[3]
ASSETS = ROOT / "crates/isle/assets/world"
RAW = ROOT / "target/isle-world-art/library"
API = "https://api.pixellab.ai/v2"
TILE = 32
CORNERS = ("NW", "NE", "SW", "SE")

# Wang tilesets. `sides` maps PixelLab's corner labels to Isle terrains as observed in the result: the path set
# came back with its labels opposite to its descriptions.
TERRAIN = {
    "shore": {"sides": {"lower": "water", "upper": "grass"}, "request": {
        "lower_description": "deep teal island sea water, small gentle wave ripples",
        "upper_description": "mossy sage-green meadow grass with sparse tiny blades",
        "transition_description": "pale sandy shore", "transition_size": 0.25,
        "lower_walkable": False, "upper_walkable": True, "seed": 72000}},
    "path": {"sides": {"lower": "dirt", "upper": "grass"}, "request": {
        "lower_description": "mossy sage-green meadow grass with sparse tiny blades",
        "upper_description": "warm earthen dirt path, sandy brown soil with tiny pebbles",
        "transition_description": "", "transition_size": 0.0,
        "lower_walkable": True, "upper_walkable": True, "seed": 72001}},
    "rocky": {"sides": {"lower": "sand", "upper": "rock"}, "request": {
        "lower_description": "pale golden beach sand, faint ripples",
        "upper_description": "rough gray rocky highland ground, angular slate stones",
        "transition_description": "", "transition_size": 0.0,
        "lower_walkable": True, "upper_walkable": True, "seed": 72004}},
    "dungeon": {"sides": {"lower": "floor", "upper": "wall"}, "request": {
        "lower_description": "light warm gray dungeon floor, pale worn flagstones with thin darker seams",
        "upper_description": "solid black-brown cave rock mass, very dark rough stone with faint cracks",
        "transition_description": "", "transition_size": 0.0,
        "lower_walkable": True, "upper_walkable": False, "seed": 72014}},
    "beach": {"sides": {"lower": "water", "upper": "sand"}, "request": {
        "lower_description": "deep teal island sea water, small gentle wave ripples",
        "upper_description": "pale golden beach sand, faint ripples",
        "transition_description": "thin white surf foam", "transition_size": 0.0,
        "lower_walkable": False, "upper_walkable": True, "seed": 72020}},
    "dunes": {"sides": {"lower": "sand", "upper": "grass"}, "request": {
        "lower_description": "pale golden beach sand, faint ripples",
        "upper_description": "mossy sage-green meadow grass with sparse tiny blades",
        "transition_description": "", "transition_size": 0.0,
        "lower_walkable": True, "upper_walkable": True, "seed": 72021}},
    "highland": {"sides": {"lower": "grass", "upper": "rock"}, "request": {
        "lower_description": "mossy sage-green meadow grass with sparse tiny blades",
        "upper_description": "rough gray rocky highland ground, angular slate stones",
        "transition_description": "", "transition_size": 0.0,
        "lower_walkable": True, "upper_walkable": True, "seed": 72022}},
}
for spec in TERRAIN.values():
    spec["request"].update(tile_size={"width": TILE, "height": TILE}, mode="standard", shape_style="round")

# Shared terrains are recolored to one reference so sets meet without seams. "rim" is the shore's sand band.
MATCH = {
    ("path", "grass"): ("shore", "grass"),
    ("rocky", "sand"): ("shore", "rim"),
    ("beach", "water"): ("shore", "water"),
    ("beach", "sand"): ("shore", "rim"),
    ("dunes", "sand"): ("shore", "rim"),
    ("dunes", "grass"): ("shore", "grass"),
    ("highland", "grass"): ("shore", "grass"),
    ("highland", "rock"): ("rocky", "rock"),
}

# The embedded object sheet, in the order the game indexes it.
OBJECTS = ["pine", "oak", "boulder", "door", "stairs_up", "stairs_down", "counter", "altar",
           "coins", "arrows", "potion", "weapon", "bow", "armor", "ring", "treasure"]
SHEET_CELLS = ["a tall dark green pine tree", "a round leafy oak tree", "a chunky gray boulder",
               "a closed oak plank door in a stone arch", "pale stone stairs going up",
               "stone stairs descending into a dark hole", "a wooden shop counter",
               "a pale stone temple altar with a gold ornament", "a small pile of gold coins", "a bundle of three arrows",
               "a red healing potion flask", "a steel sword", "a curved wooden bow", "a steel breastplate",
               "a gold ring with a turquoise gem", "a small wooden treasure chest with gold bands"]
SINGLE_STYLE = ("Pixel art game object, centered with a clear margin, transparent background, high top-down RPG view, "
                "clean dark outlines, muted natural palette, matching a classic 32x32 tile set. ")
# Sheet cells that came out wrong are replaced by single images.
SINGLES = {
    "stairs_down": ("a square dark pit in the ground with pale stone steps leading down into black darkness, "
                    "viewed from above", 73100),
    "coins": ("a small loose pile of shiny round gold coins on the ground, no box or container", 73101),
    "armor": ("a steel breastplate chest armor piece with shoulder plates, lying flat", 73102),
}


def library():
    entries = {f"terrain-{name}": ("/create-tileset", spec["request"]) for name, spec in TERRAIN.items()}
    entries["objects"] = ("/create-image-pixen", {
        "description": ("Pixel art sprite sheet, a strict 4 by 4 grid of 16 separate game objects, each centered in "
                        "its own 32x32 cell, not touching neighbors, transparent background, high top-down RPG view, "
                        "clean dark outlines, muted natural palette. Row by row, left to right: "
                        + "; ".join(SHEET_CELLS) + "."),
        "image_size": {"width": 128, "height": 128}, "no_background": True,
        "outline": "single color black outline", "detail": "medium detail", "view": "high top-down", "seed": 73001})
    for name, (subject, seed) in SINGLES.items():
        entries[f"object-{name}"] = ("/create-image-pixen", {
            "description": SINGLE_STYLE + subject, "image_size": {"width": TILE, "height": TILE},
            "no_background": True, "outline": "single color black outline", "detail": "medium detail",
            "view": "high top-down", "seed": seed})
    return entries


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None  # Never forward the authorization header to another host.


def api(path, body=None):
    request = urllib.request.Request(
        API + path, data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": "Bearer " + os.environ["PIXELLAB_SECRET"], "Content-Type": "application/json"})
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=300) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        # A failed POST is never retried automatically: it may already have spent a generation.
        raise RuntimeError(f"PixelLab {path}: HTTP {error.code}; inspect before retrying") from None


def sha(data):
    return hashlib.sha256(data).hexdigest()


def remaining():
    return api("/balance").get("subscription", {}).get("generations", 0)


def collect(name, endpoint, job, tileset_id, deadline):
    """Poll a submitted job. Returns the finished response, or None if it is still running at the deadline."""
    while True:
        status = api(f"/background-jobs/{job}")
        if status["status"] == "failed":
            raise RuntimeError(f"{name}: PixelLab job failed; inspect {name}/pending.json")
        if status["status"] == "completed":
            if endpoint == "/create-tileset":
                result = api(f"/tilesets/{tileset_id}")
            else:
                result = status["last_response"]
            result["usage"] = status.get("usage") or result.get("usage")
            return result
        if time.monotonic() > deadline:
            return None
        time.sleep(8)


def generate(names, budget, wait=1800):
    entries = library()
    unknown = [n for n in names if n not in entries]
    if unknown:
        raise RuntimeError(f"unknown library entries: {', '.join(unknown)}")
    spent = 0
    available = remaining()
    print(f"PixelLab: {available:g} generations available; this run may spend {budget:g}", flush=True)
    for name in names:
        endpoint, request = entries[name]
        folder = RAW / name
        original, receipt, pending = folder / "original.json", folder / "receipt.json", folder / "pending.json"
        if receipt.exists():
            print(f"{name}: already in the library", flush=True)
            continue
        folder.mkdir(parents=True, exist_ok=True)
        if pending.exists():
            state = json.loads(pending.read_text())
            if "background_job_id" not in state:
                raise RuntimeError(f"{name}: an earlier request may have been charged; inspect {name}/pending.json")
            print(f"{name}: resuming job {state['background_job_id']}", flush=True)
        else:
            if spent + 1 > budget or available < 1:
                print("Budget for this run reached; no further requests sent", flush=True)
                return
            pending.write_text(json.dumps({"state": "submitting", "endpoint": endpoint, "request": request}) + "\n")
            response = api(endpoint, request)
            spent += 1
            if "background_job_id" not in response:
                state = {"done": response}
            else:
                state = {"endpoint": endpoint, "request": request,
                         "background_job_id": response["background_job_id"],
                         "tileset_id": response.get("tileset_id")}
                pending.write_text(json.dumps(state) + "\n")
        result = state.get("done") or collect(
            name, endpoint, state["background_job_id"], state.get("tileset_id"), time.monotonic() + wait)
        if result is None:
            print(f"{name}: still running; rerun later to collect it (it will not be resubmitted)", flush=True)
            continue
        data = json.dumps(result).encode()
        original.write_bytes(data)
        validate(name, endpoint, result)
        usage = result.get("usage") or {}
        receipt.write_text(json.dumps({"provider": "PixelLab", "endpoint": endpoint, "request": request,
                                       "usage": usage, "original_sha256": sha(data)}, indent=2) + "\n")
        pending.unlink()
        available = remaining()
        print(f"{name}: saved; charged {usage.get('generations', '?')}, {available:g} remain", flush=True)


def images(result):
    """Decoded images of a stored response: one per tile for tilesets, else the single image."""
    if "tileset" in result:
        found = [(t["corners"], t["image"]["base64"]) for t in result["tileset"]["tiles"]]
    else:
        found = [(None, result["image"]["base64"])]
    return [(corners, Image.open(io.BytesIO(base64.b64decode(b64.split(",")[-1]))).convert("RGBA"))
            for corners, b64 in found]


def validate(name, endpoint, result):
    decoded = images(result)
    size = endpoint == "/create-tileset" and (TILE, TILE) or None
    if endpoint == "/create-tileset":
        combos = {tuple(c[k] for k in CORNERS) for c, _ in decoded}
        if len(decoded) != 16 or len(combos) != 16:
            raise RuntimeError(f"{name}: expected 16 distinct corner tiles")
    for _, image in decoded:
        if size and image.size != size:
            raise RuntimeError(f"{name}: unexpected tile size {image.size}")


def load(name):
    folder = RAW / name
    receipt = json.loads((folder / "receipt.json").read_text())
    data = (folder / "original.json").read_bytes()
    if sha(data) != receipt["original_sha256"]:
        raise RuntimeError(f"{name}: original no longer matches its receipt")
    return receipt, json.loads(data)


def colors(image):
    return set(image.get_flattened_data())


def dominant(pixels):
    return Counter(p for p in pixels if p[3]).most_common(1)[0][0]


def distance(a, b):
    return sum((a[i] - b[i]) ** 2 for i in range(3))


def recolor(tiles, keep, other, reference, base):
    """Map one terrain's colors onto a reference terrain, shifting by the difference of their main colors.
    Pixels nearer the set's other terrain are left alone."""
    mapping = {}
    for image in tiles.values():
        for c in colors(image):
            if c[3] and distance(c, keep) < distance(c, other):
                shifted = tuple(max(0, min(255, c[i] - keep[i] + base[i])) for i in range(3))
                mapping[c] = (min(reference, key=lambda q: distance(q, shifted)) if distance(c, keep) < 900
                              else (*shifted, c[3]))
    return mapping


def terrain_tiles(result, sides, b):
    """Tiles keyed by a 4-bit mask, NW = 8 … SE = 1, where a set bit marks the corner as terrain `b`."""
    tiles = {}
    for corners, image in images(result):
        mask = sum(8 >> i for i, k in enumerate(CORNERS) if sides[corners[k]] == b)
        tiles[mask] = image
    return tiles


def process():
    ASSETS.mkdir(parents=True, exist_ok=True)
    manifest = {"provider": "PixelLab", "tile_size": TILE, "terrain": {}, "objects": {}, "sources": {}}
    sets = {}
    for name, spec in TERRAIN.items():
        receipt, result = load(f"terrain-{name}")
        manifest["sources"][f"terrain-{name}"] = receipt
        a, b = spec["sides"]["lower"], spec["sides"]["upper"]
        sets[name] = (a, b, terrain_tiles(result, spec["sides"], b))
    references = {}
    for name, (a, b, tiles) in sets.items():
        for terrain, full in ((a, tiles[0]), (b, tiles[15])):
            references[(name, terrain)] = (colors(full), dominant(full.get_flattened_data()))
    water, grass = references[("shore", "water")][0], references[("shore", "grass")][0]
    rim = [p for image in sets["shore"][2].values() for p in image.get_flattened_data() if p not in water | grass]
    references[("shore", "rim")] = (set(rim), dominant(rim))
    for (name, terrain), source in MATCH.items():
        a, b, tiles = sets[name]
        keep, other = (tiles[0], tiles[15]) if terrain == a else (tiles[15], tiles[0])
        reference, base = references[source]
        mapping = recolor(tiles, dominant(keep.get_flattened_data()), dominant(other.get_flattened_data()),
                          reference, base)
        for mask, image in tiles.items():
            image.putdata([mapping.get(p, p) for p in image.get_flattened_data()])
    for name, (a, b, tiles) in sets.items():
        sheet = Image.new("RGBA", (4 * TILE, 4 * TILE))
        for mask, image in tiles.items():
            sheet.paste(image, ((mask % 4) * TILE, (mask // 4) * TILE))
        file = ASSETS / f"terrain_{name}.png"
        sheet.save(file)
        manifest["terrain"][name] = {"a": a, "b": b, "file": file.name, "image_sha256": sha(file.read_bytes()),
                                     "layout": "4x4 by corner mask: NW=8, NE=4, SW=2, SE=1 set where terrain b"}
    receipt, result = load("objects")
    manifest["sources"]["objects"] = receipt
    (_, sheet), = images(result)
    if sheet.size != (4 * TILE, 4 * TILE):
        raise RuntimeError(f"objects: unexpected sheet size {sheet.size}")
    for name in SINGLES:
        receipt, result = load(f"object-{name}")
        manifest["sources"][f"object-{name}"] = receipt
        (_, single), = images(result)
        i = OBJECTS.index(name)
        sheet.paste((0, 0, 0, 0), ((i % 4) * TILE, (i // 4) * TILE, (i % 4 + 1) * TILE, (i // 4 + 1) * TILE))
        sheet.paste(single, ((i % 4) * TILE, (i // 4) * TILE))
    alpha = sheet.getchannel("A")
    for k in range(1, 4):
        edges = [(k * TILE - 1, t) for t in range(4 * TILE)] + [(k * TILE, t) for t in range(4 * TILE)]
        if any(alpha.getpixel(p) or alpha.getpixel(p[::-1]) for p in edges):
            raise RuntimeError("objects: an object crosses its cell boundary")
    sheet.putalpha(alpha.point(lambda v: 255 if v >= 128 else 0))
    file = ASSETS / "objects.png"
    sheet.save(file)
    manifest["objects"] = {"file": file.name, "order": OBJECTS, "image_sha256": sha(file.read_bytes())}
    (ASSETS / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Processed {len(sets)} terrain sets and {len(OBJECTS)} objects")


def contact():
    """Review images: each terrain set laid out as an island, plus the object sheet."""
    manifest = json.loads((ASSETS / "manifest.json").read_text())
    shape = ["............", "....####....", "..########..", ".##########.", ".#########..",
             "..#######...", "....###.....", "............"]
    corner = lambda x, y: 0 <= y < len(shape) and 0 <= x < 12 and shape[y][x] == "#"
    rows = []
    for name, info in manifest["terrain"].items():
        with Image.open(ASSETS / info["file"]) as sheet:
            sheet = sheet.convert("RGBA")
        image = Image.new("RGBA", (11 * TILE, 7 * TILE))
        for y in range(7):
            for x in range(11):
                mask = corner(x, y) * 8 | corner(x + 1, y) * 4 | corner(x, y + 1) * 2 | corner(x + 1, y + 1)
                image.paste(sheet.crop(((mask % 4) * TILE, (mask // 4) * TILE, (mask % 4 + 1) * TILE,
                                        (mask // 4 + 1) * TILE)), (x * TILE, y * TILE))
        rows.append((name, image))
    with Image.open(ASSETS / manifest["objects"]["file"]) as objects:
        rows.append(("objects", objects.convert("RGBA")))
    width = 2 * max(i.width for _, i in rows) + 16
    out = Image.new("RGBA", (width, sum(2 * i.height + 24 for _, i in rows)), "#171b2b")
    draw, y = ImageDraw.Draw(out), 0
    for name, image in rows:
        draw.text((8, y + 4), name, fill="#f3e5bc")
        big = image.resize((2 * image.width, 2 * image.height), Image.Resampling.NEAREST)
        out.alpha_composite(big, (8, y + 20))
        y += big.height + 24
    RAW.mkdir(parents=True, exist_ok=True)
    out.save(RAW / "contact.png")
    print(RAW / "contact.png")


def status():
    for name in library():
        folder = RAW / name
        state = ("ready" if (folder / "receipt.json").exists() else
                 "pending" if (folder / "pending.json").exists() else "wanted")
        print(f"{name:24} {state}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    create = sub.add_parser("generate")
    create.add_argument("names", nargs="+")
    create.add_argument("--budget", type=float, default=5, help="Most new generations this run may spend")
    create.add_argument("--wait", type=float, default=1800, help="Seconds to wait for a job before leaving it")
    for command in ("process", "contact", "status"):
        sub.add_parser(command)
    args = parser.parse_args()
    if args.command == "generate":
        generate(args.names, args.budget, args.wait)
    else:
        {"process": process, "contact": contact, "status": status}[args.command]()
