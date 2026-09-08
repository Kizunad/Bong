"""将已认可的第二轮槽框与残环导出为生产 SVG；保留原稿作为人工对照。"""

from make_assets import Art, ROOT, GOLD, cast


def export():
    destination = ROOT.parents[1] / "src/main/resources/assets/bong-client/svg/hud"
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("quick-slot", "quick-slot-selected", "cast-complete", "cast-interrupted"):
        (destination / f"{name}.svg").write_bytes((ROOT / "round-2" / f"{name}.svg").read_bytes())
    cast(True, 0).save(destination, "cast-track")
    for index in range(12):
        segment = Art(88, 88)
        start = index * 30 - 88
        segment.arc(44, 44, 37, start, start + 18, GOLD, 1.5, .9)
        segment.save(destination, f"cast-segment-{chr(ord('a') + index)}")


if __name__ == "__main__":
    export()
