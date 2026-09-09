"""按第二轮反馈生成黄色人体和六种伤势预览；不接入生产 HUD。"""

from make_assets import ROOT, append_body_meters, mini_body_frame

YELLOW = "#e8c64d"
WOUNDS = (
    ("INTACT", "完好", "完整剪影"),
    ("BRUISE", "淤伤", "局部暗斑"),
    ("ABRASION", "擦伤", "浅表擦痕"),
    ("LACERATION", "割裂", "单道开口"),
    ("FRACTURE", "骨折", "折线断纹"),
    ("SEVERED", "断肢", "前臂与手缺失"),
)


def yellow_body(wound):
    a = mini_body_frame(True)
    a.ellipse(47, 23.5, 7, 9.5, YELLOW)
    shoulder = [(51, 31), (51, 35), (57, 37), (61, 40),
                (63, 46), (65, 54), (67, 62)]
    forearm = [(69, 70), (71, 78), (72, 81), (71.5, 85), (69.5, 86),
               (67, 84), (65.5, 80), (65, 76), (63, 70), (61, 64)]
    trunk_leg = [(59, 58), (56, 50), (55, 55), (54, 64), (55, 70),
                 (57, 76), (57.5, 83), (58, 94), (57, 105), (58, 120),
                 (58, 125), (62, 128), (63, 131), (61, 133), (55, 133),
                 (52, 129), (51.5, 121), (50, 108), (49, 100), (47, 87)]
    # 正面图的左前臂在画面右侧；断肢直接省略远端轮廓，不用背景色遮盖。
    intact_half = shoulder + forearm + trunk_leg
    affected_forearm = (
        [(67.2, 64), (65, 65), (63.5, 64), (62, 66), (61, 64)]
        if wound == "SEVERED" else forearm
    )
    outline = shoulder + affected_forearm + trunk_leg
    outline += [(94 - x, y) for x, y in reversed(intact_half[:-1])]
    a.poly(outline, YELLOW)

    if wound == "BRUISE":
        a.poly([(62, 65), (65.5, 65), (68, 70), (69.5, 76),
                (68, 79), (65.5, 76), (64, 71)], "#9b7357")
        a.poly([(63.5, 68), (66, 67), (68, 72), (67, 76), (65, 73)], "#774f59")
    elif wound == "ABRASION":
        a.poly([(62, 66), (66, 65), (69.5, 77), (66, 79)], "#c99745")
        for x, y in [(62.8, 68), (63.6, 71), (64.5, 74), (65.3, 77)]:
            a.line(x, y, x + 3.7, y - 1.6, "#a95538", 1.3)
    elif wound == "LACERATION":
        a.line(63.8, 74, 68.4, 69, "#c56b46", 3)
        a.line(63.8, 74, 68.4, 69, "#552c2c", 1.4)
    elif wound == "FRACTURE":
        a.poly([(61.5, 65), (66.7, 64.5), (70, 76), (66, 79), (64, 72)], "#c57654")
        for start, end in [((63, 68.5), (66.5, 70)),
                           ((66.5, 70), (64.5, 73)),
                           ((64.5, 73), (69, 74.5))]:
            a.line(*start, *end, "#502f2d", 1.8)
    elif wound == "SEVERED":
        a.poly([(60.8, 63), (66.6, 62), (67.2, 64),
                (65, 65), (63.5, 64), (62, 66), (61, 64)], "#a85143")
    elif wound != "INTACT":
        raise ValueError(f"未设计的伤势: {wound}")

    append_body_meters(a)
    return a


def generate():
    folder = ROOT / "round-2-wounds"
    folder.mkdir(parents=True, exist_ok=True)
    for wound, _, _ in WOUNDS:
        yellow_body(wound).save(folder, f"mini-body-{wound.lower()}")
    print(f"已生成 {len(WOUNDS)} 种伤势 SVG: {folder}")


if __name__ == "__main__":
    generate()
