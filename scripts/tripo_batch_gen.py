#!/usr/bin/env python3
"""Batch generate 3D models via Tripo3D API for the Bong project.

Usage:
    export TRIPO_API_KEY=tsk_xxx
    python scripts/tripo_batch_gen.py [--dry-run] [--only NAME] [--convert obj]
"""

import argparse
import asyncio
import json
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import httpx

BASE_URL = "https://api.tripo3d.ai/v2/openapi"
OUTPUT_DIR = Path("local_models/tripo_generated")
MAX_CONCURRENT = 8  # stay under plan limit
POLL_INTERVAL = 5  # seconds


@dataclass
class ModelSpec:
    name: str
    prompt: str
    category: str
    negative_prompt: str = "low quality, blurry, modern, sci-fi, cartoon"
    face_limit: int = 50000


# ── Model List ──────────────────────────────────────────────────────────
# Prioritized by project need: weapons > spirit treasures > alchemy > creatures > props
MODELS: list[ModelSpec] = [
    ModelSpec(
        name="horsetail_whisk",
        category="weapon",
        prompt="Taoist horsehair fly whisk (拂尘), long bamboo handle with flowing white horsehair tassel, jade ring at the joint, traditional Chinese Taoist weapon, elegant and simple, single item on plain background",
    ),
    ModelSpec(
        name="iron_war_fan",
        category="weapon",
        prompt="Chinese iron war fan (铁扇), partially open folding fan with sharp metal ribs and dark silk fabric, engraved with faded rune patterns, martial arts weapon, single item on plain background",
    ),
    ModelSpec(
        name="formation_flag",
        category="prop",
        prompt="Ancient Chinese formation flag (阵旗), small triangular banner on a short iron pole, the fabric is dark red silk with golden rune circle embroidered, weathered and frayed edges, xianxia ritual item, single item on plain background",
        face_limit=30000,
    ),
    ModelSpec(
        name="talisman_paper",
        category="prop",
        prompt="Chinese yellow paper talisman (符箓), rectangular strip of aged yellow paper with red cinnabar brush-painted rune characters and seal marks, slightly curled edges, traditional Taoist charm, single item on plain background",
        face_limit=20000,
    ),
    ModelSpec(
        name="spirit_herb",
        category="alchemy",
        prompt="Single glowing spirit herb (灵草), a small luminescent plant with pale blue-green crystalline leaves growing from dark soil clump, faintly translucent, xianxia medicinal herb, single item on plain background",
        face_limit=20000,
    ),
    ModelSpec(
        name="herb_knife",
        category="weapon",
        prompt="Simple herb gathering knife (采药刀), short curved blade with wooden handle wrapped in hemp cord, rustic and well-worn, traditional Chinese herbalist tool, single item on plain background",
        face_limit=30000,
    ),
    ModelSpec(
        name="dead_drop_box",
        category="prop",
        prompt="Locked wooden storage box (死信箱) half-buried in dirt, small reinforced chest with iron bands and a rusted padlock, faded rune symbols carved on lid, dark wood weathered, xianxia anonymous trade container, single item on plain background",
        face_limit=40000,
    ),
    ModelSpec(
        name="spirit_stone_cluster",
        category="alchemy",
        prompt="Cluster of three spirit stones (灵石), translucent crystalline stones with faint inner glow, pale blue-white color, rough natural facets, sitting on dark rock base, xianxia energy crystals, single item on plain background",
        face_limit=30000,
    ),
    ModelSpec(
        name="ancient_scroll",
        category="prop",
        prompt="Partially unrolled ancient Chinese scroll (残卷), aged yellowed paper with faded ink calligraphy visible, wooden scroll rods at both ends, slightly torn and damaged, cultivation technique manual fragment, single item on plain background",
        face_limit=30000,
    ),
    ModelSpec(
        name="jade_pendant",
        category="spirit_treasure",
        prompt="Chinese jade pendant (玉佩), flat circular disc of translucent green jade with carved dragon motif, attached to red silk cord with decorative knots, polished surface, xianxia protective amulet, single item on plain background",
        face_limit=30000,
    ),
]


def headers() -> dict[str, str]:
    key = os.environ.get("TRIPO_API_KEY", "")
    if not key:
        print("ERROR: TRIPO_API_KEY not set", file=sys.stderr)
        sys.exit(1)
    return {
        "Authorization": f"Bearer {key}",
        "Content-Type": "application/json",
    }


async def check_balance(client: httpx.AsyncClient) -> dict:
    r = await client.get(f"{BASE_URL}/user/balance", headers=headers())
    data = r.json()
    if data.get("code") != 0:
        print(f"Balance check failed: {data}", file=sys.stderr)
        sys.exit(1)
    return data["data"]


async def create_task(client: httpx.AsyncClient, spec: ModelSpec) -> str:
    payload = {
        "type": "text_to_model",
        "prompt": spec.prompt,
        "negative_prompt": spec.negative_prompt,
        "model_version": "v2.5-20250123",
        "texture": True,
        "pbr": True,
        "texture_quality": "standard",
        "face_limit": spec.face_limit,
    }
    r = await client.post(f"{BASE_URL}/task", headers=headers(), json=payload)
    data = r.json()
    if data.get("code") != 0:
        raise RuntimeError(f"Task creation failed for {spec.name}: {data}")
    task_id = data["data"]["task_id"]
    print(f"  [{spec.name}] task created: {task_id}")
    return task_id


async def poll_task(client: httpx.AsyncClient, task_id: str, name: str) -> dict:
    while True:
        r = await client.get(f"{BASE_URL}/task/{task_id}", headers=headers())
        data = r.json()["data"]
        status = data.get("status", "unknown")
        progress = data.get("progress", 0)

        if status == "success":
            print(f"  [{name}] ✅ done")
            return data
        elif status in ("failed", "cancelled", "banned"):
            print(f"  [{name}] ❌ {status}")
            return data
        else:
            print(f"  [{name}] {status} {progress}%", end="\r")
            await asyncio.sleep(POLL_INTERVAL)


async def download_file(client: httpx.AsyncClient, url: str, dest: Path):
    r = await client.get(url, follow_redirects=True, timeout=120)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(r.content)
    size_kb = len(r.content) / 1024
    print(f"    saved {dest.name} ({size_kb:.0f} KB)")


async def convert_model(
    client: httpx.AsyncClient, task_id: str, fmt: str, name: str
) -> str | None:
    payload = {
        "type": "convert_model",
        "original_model_task_id": task_id,
        "format": fmt.upper(),
        "texture_size": 2048,
        "pivot_to_center_bottom": True,
    }
    r = await client.post(f"{BASE_URL}/task", headers=headers(), json=payload)
    data = r.json()
    if data.get("code") != 0:
        print(f"  [{name}] convert to {fmt} failed: {data}")
        return None
    convert_id = data["data"]["task_id"]
    print(f"  [{name}] converting to {fmt}...")
    result = await poll_task(client, convert_id, f"{name}-{fmt}")
    if result.get("status") == "success":
        return result["output"].get("model")
    return None


async def process_one(
    client: httpx.AsyncClient,
    spec: ModelSpec,
    sem: asyncio.Semaphore,
    convert_fmt: str | None,
    dry_run: bool,
):
    async with sem:
        out_dir = OUTPUT_DIR / spec.category / spec.name
        out_dir.mkdir(parents=True, exist_ok=True)

        if dry_run:
            print(f"  [DRY RUN] {spec.name}: {spec.prompt[:80]}...")
            meta = {"name": spec.name, "category": spec.category, "prompt": spec.prompt}
            (out_dir / "meta.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))
            return

        task_id = await create_task(client, spec)
        result = await poll_task(client, task_id, spec.name)

        if result.get("status") != "success":
            print(f"  [{spec.name}] FAILED — skipping download")
            return

        output = result.get("output", {})
        meta = {
            "name": spec.name,
            "category": spec.category,
            "prompt": spec.prompt,
            "task_id": task_id,
            "timestamp": int(time.time()),
        }

        # Download GLB
        if output.get("pbr_model"):
            await download_file(client, output["pbr_model"], out_dir / f"{spec.name}_pbr.glb")
        if output.get("model"):
            await download_file(client, output["model"], out_dir / f"{spec.name}.glb")
        if output.get("rendered_image"):
            await download_file(client, output["rendered_image"], out_dir / f"{spec.name}_preview.webp")

        # Convert if requested
        if convert_fmt:
            url = await convert_model(client, task_id, convert_fmt, spec.name)
            if url:
                ext = convert_fmt.lower()
                if ext == "fbx":
                    ext = "zip"  # FBX comes as zip
                await download_file(client, url, out_dir / f"{spec.name}.{ext}")

        (out_dir / "meta.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))


async def main():
    parser = argparse.ArgumentParser(description="Batch generate 3D models via Tripo3D")
    parser.add_argument("--dry-run", action="store_true", help="Print plan without generating")
    parser.add_argument("--only", type=str, help="Generate only this model name")
    parser.add_argument("--convert", type=str, help="Also convert to format (obj, fbx)")
    parser.add_argument("--list", action="store_true", help="List all model specs")
    args = parser.parse_args()

    models = MODELS
    if args.only:
        models = [m for m in MODELS if m.name == args.only]
        if not models:
            print(f"Model '{args.only}' not found. Available: {[m.name for m in MODELS]}")
            sys.exit(1)

    if args.list:
        for m in models:
            print(f"  {m.category:16s} {m.name:24s} faces≤{m.face_limit}")
        print(f"\nTotal: {len(models)} models, est. {len(models) * 40} credits")
        return

    async with httpx.AsyncClient(timeout=30) as client:
        bal = await check_balance(client)
        available = bal["balance"] - bal["frozen"]
        est_cost = len(models) * 40
        print(f"Balance: {bal['balance']} (frozen: {bal['frozen']}, available: {available})")
        print(f"Models: {len(models)}, estimated cost: ~{est_cost} credits")

        if not args.dry_run and est_cost > available:
            print(f"⚠️  Estimated cost ({est_cost}) > available ({available}). Proceeding anyway (actual cost may differ).")

        print(f"\n{'='*60}")
        print("Model generation plan:")
        for m in models:
            print(f"  {m.category:16s} {m.name:24s} — {m.prompt[:60]}...")
        print(f"{'='*60}\n")

        if args.dry_run:
            print("DRY RUN — no tasks will be created\n")

        sem = asyncio.Semaphore(MAX_CONCURRENT)
        tasks = [process_one(client, spec, sem, args.convert, args.dry_run) for spec in models]
        await asyncio.gather(*tasks)

        if not args.dry_run:
            bal2 = await check_balance(client)
            print(f"\nFinal balance: {bal2['balance']} (spent: {bal['balance'] - bal2['balance']})")

    print(f"\nOutput directory: {OUTPUT_DIR}")
    print("Done!")


if __name__ == "__main__":
    asyncio.run(main())
