"""生成 HUD 美术预览；只使用当前生产解析器支持的 SVG 图元。"""

from math import cos, sin, radians, hypot
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK, BONE, DIM = "#171b19", "#d6cab0", "#6c7266"
GOLD, BLOOD, QI = "#ba996b", "#b86654", "#7fa99a"


class Art:
    def __init__(self, width, height):
        self.width, self.height = width, height
        self.parts = []

    def poly(self, points, color, opacity=1):
        pts = " ".join(f"{x:.3f},{y:.3f}" for x, y in points)
        self.parts.append(f'<polygon points="{pts}" fill="{color}" opacity="{opacity}"/>')

    def line(self, x1, y1, x2, y2, color, width=1, opacity=1):
        length = hypot(x2 - x1, y2 - y1)
        nx, ny = -(y2 - y1) / length * width / 2, (x2 - x1) / length * width / 2
        self.poly([(x1 + nx, y1 + ny), (x2 + nx, y2 + ny),
                   (x2 - nx, y2 - ny), (x1 - nx, y1 - ny)], color, opacity)

    def ellipse(self, x, y, rx, ry, color, opacity=1):
        self.parts.append(f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{color}" opacity="{opacity}"/>')

    def arc(self, x, y, radius, start, end, color, width=1, opacity=1):
        steps = max(2, int(abs(end - start) / 6))
        angles = [radians(start + (end - start) * i / steps) for i in range(steps + 1)]
        outer = [(x + (radius + width / 2) * cos(t), y + (radius + width / 2) * sin(t)) for t in angles]
        inner = [(x + (radius - width / 2) * cos(t), y + (radius - width / 2) * sin(t)) for t in reversed(angles)]
        self.poly(outer + inner, color, opacity)

    def diamond(self, x, y, radius, color, opacity=1):
        self.poly([(x, y - radius), (x + radius * .65, y), (x, y + radius), (x - radius * .65, y)], color, opacity)

    def save(self, folder, name):
        text = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {self.width} {self.height}">\n'
        text += '\n'.join(self.parts) + '\n</svg>\n'
        (folder / (name + '.svg')).write_text(text, encoding='utf-8')


def mini_body_frame(refined):
    a = Art(112, 148)
    a.poly([(21, 6), (69, 4), (78, 15), (75, 55), (78, 109), (67, 138), (24, 140), (15, 116), (18, 62)], INK, .64)
    if refined:
        a.line(24, 8, 66, 6, DIM, .6, .6)
        a.line(21, 134, 62, 137, DIM, .6, .4)
        a.arc(47, 70, 42, 105, 258, DIM, .6, .4)
        for y in (24, 49, 74, 99, 124):
            a.line(14, y, 17, y, BONE, .7, .55)
    return a


def mini_body(refined, wounded=False):
    a = mini_body_frame(refined)
    # 手绘轮廓用填充多边形表达；后续运行时必须从 BodyPlan 的部位锚点投影。
    a.ellipse(47, 24, 7.3, 9, BONE, .75)
    a.poly([(43, 32), (51, 32), (53, 36), (60, 39), (56, 52), (55, 66),
            (58, 78), (52, 84), (43, 83), (37, 78), (40, 65), (39, 51), (34, 39), (41, 36)], BONE, .67)
    for points in (
        [(35, 39), (40, 44), (32, 65), (28, 81), (24, 88), (22, 85), (26, 63)],
        [(60, 39), (54, 44), (62, 65), (65, 82), (69, 87), (72, 83), (68, 62)],
        [(38, 78), (47, 83), (43, 102), (41, 125), (37, 134), (30, 134), (35, 125), (35, 102)],
        [(48, 83), (58, 78), (59, 101), (59, 125), (65, 132), (64, 134), (56, 133), (53, 123), (51, 104)]
    ):
        a.poly(points, BONE, .58)
    for start, end in [((47, 38), (47, 65)), ((47, 48), (39, 45)), ((47, 48), (55, 45)),
                       ((41, 61), (47, 64)), ((53, 61), (47, 64)), ((47, 65), (47, 76))]:
        a.line(*start, *end, INK, 1.3, .8)
    if refined:
        for y in range(51, 61, 3):
            a.line(42, y, 46, y + 1.2, INK, .7, .55)
            a.line(49, y + 1.2, 53, y, INK, .7, .55)
        a.line(30, 65, 26, 82, QI, .8, .75)
        a.line(63, 65, 67, 81, QI, .8, .6)
        a.line(40, 90, 38, 124, QI, .8, .6)
        a.line(54, 90, 56, 124, QI, .8, .6)
        a.ellipse(47, 70, 2.4, 3.2, QI, .85)
        for x, y in [(30, 65), (63, 65), (39, 103), (55, 103)]:
            a.ellipse(x, y, 1.3, 1.3, GOLD, .8)
    if wounded:
        a.poly([(27, 63), (35, 59), (31, 69), (25, 78), (25, 72), (22, 77)], BLOOD, .95)
        a.line(26, 67, 33, 65, BONE, .65)
        a.line(25, 70, 31, 68, BONE, .65)
        a.diamond(20, 59, 2, BLOOD)
    append_body_meters(a)
    return a


def append_body_meters(a):
    # 两条细刻度对应现有真元和体力，不新增生命数值。
    for x, color, filled in [(87, QI, 7), (103, GOLD, 9)]:
        a.line(x, 29, x, 124, INK, 5, .85)
        for n in range(11):
            yy = 121 - n * 8.7
            a.poly([(x - 2, yy), (x, yy - 2), (x + 2, yy), (x, yy + 3)], color if n < filled else DIM, .85 if n < filled else .3)
        a.diamond(x, 20, 4, color, .75)
        a.line(x - 3, 130, x + 3, 130, BONE, .8, .5)


def slot(refined, selected=False):
    a = Art(48, 52)
    a.poly([(7, 4), (38, 3), (44, 10), (43, 43), (37, 48), (8, 47), (3, 41), (4, 10)], INK, .93)
    a.poly([(8, 7), (37, 6), (41, 11), (40, 42), (35, 44), (9, 44), (6, 40), (7, 12)], "#41443a", .68)
    a.poly([(10, 10), (35, 9), (38, 13), (37, 39), (33, 41), (11, 40), (9, 37)], INK, .55)
    edge = GOLD if selected else DIM
    a.line(8, 5, 36, 4, edge, 1, .85)
    a.line(43, 12, 42, 40, edge, 1, .75)
    a.line(11, 46, 36, 47, edge, 1, .7)
    if refined:
        for x, y in [(6, 14), (6, 17), (40, 32), (40, 35)]:
            a.line(x - 2, y - 1, x + 2, y + 1, BONE, .7, .65)
        a.line(12, 8, 29, 7.5, BONE, .6, .22)
        a.line(36, 39, 32, 43, BONE, .6, .3)
        a.poly([(8, 46), (12, 45), (11, 49)], "#4d493c", .85)
    if selected:
        a.line(7, 11, 7, 34, BONE, 1.4)
        a.diamond(24, 3, 2.7, BONE)
        a.poly([(18, 49), (24, 47), (30, 49), (24, 51)], GOLD)
    return a


def cast(refined, progress=.65, state="casting"):
    a = Art(88, 88)
    complete = state == "complete"
    interrupted = state == "interrupted"
    color = BLOOD if interrupted else BONE if complete else GOLD
    radius = 26 if complete else 30 + (1 - progress) * 7
    for n in range(12):
        start = n * 30 - 88
        a.arc(44, 44, radius, start, start + 21, DIM, .7, .38)
        if n / 12 < progress:
            radius_n = radius + (3 if interrupted and n % 2 else 0)
            a.arc(44, 44, radius_n, start, start + 18, color, 1.5, .9)
    if refined:
        for n in range(4):
            t = radians(n * 90)
            r = radius - 6
            a.line(44 + r * cos(t), 44 + r * sin(t),
                   44 + (r + 3) * cos(t), 44 + (r + 3) * sin(t), color, .8, .65)
        a.arc(44, 44, radius - 4, 125, 170, color, .7, .35)
        a.arc(44, 44, radius - 4, 305, 350, color, .7, .35)
    if complete:
        a.diamond(44, 44, 6, BONE)
        a.diamond(44, 44, 3.3, INK)
    elif interrupted:
        a.line(36, 49, 42, 43, BLOOD, 1.5)
        a.line(47, 39, 52, 34, BLOOD, 1.5)
    else:
        # 中心透空；正式接线需避开准星与截脉环。
        for x, direction in [(37, -1), (51, 1)]:
            a.poly([(x, 43), (x + direction * 3, 40), (x + direction * 3, 46)], color, .7)
    return a


def target_hp(refined):
    a = Art(208, 40)
    a.line(18, 22, 189, 22, INK, 6, .72)
    for i in range(26):
        x = 23 + i * 6
        h = 5 + (2 if i % 5 == 0 else 0)
        a.line(x, 22 - h / 2, x, 22 + h / 2, BLOOD if i < 17 else DIM, 1.7, .9 if i < 17 else .42)
    a.diamond(11, 22, 5, BLOOD)
    a.diamond(198, 22, 3, DIM)
    if refined:
        a.line(22, 14, 58, 13, BLOOD, .6, .55)
        a.line(156, 30, 184, 29, BONE, .6, .45)
    return a


def target_name(refined):
    a = Art(208, 48)
    a.poly([(29, 12), (169, 10), (180, 16), (176, 34), (35, 38), (26, 31)], INK, .83)
    a.line(35, 12, 169, 10, BONE, .7, .6)
    a.line(39, 36, 172, 34, BONE, .6, .4)
    a.poly([(5, 24), (16, 17), (27, 24), (16, 30)], BONE, .75)
    a.poly([(8, 24), (16, 21), (24, 24), (16, 27)], INK)
    a.ellipse(16, 24, 1.7, 2.5, GOLD)
    if refined:
        a.line(185, 17, 194, 14, BONE, .8, .7)
        a.line(188, 22, 200, 20, BONE, .8, .6)
        a.line(185, 29, 194, 31, BONE, .8, .45)
    return a


def target_realm(refined):
    a = Art(112, 88)
    a.diamond(56, 44, 35, INK, .82)
    for r in [23, 29, 35]:
        a.line(56 - r * .65, 44, 56, 44 - r, GOLD, .8, .8)
        a.line(56, 44 + r, 56 + r * .65, 44, GOLD, .8, .6)
    if refined:
        a.diamond(56, 9, 2, BONE)
        a.line(21, 44, 27, 44, GOLD, 1)
        a.line(85, 44, 91, 44, GOLD, 1)
        a.line(56, 79, 56, 83, GOLD, 1)
    return a


def target_qi(refined):
    a = Art(88, 88)
    a.ellipse(44, 44, 29, 29, INK, .6)
    for n in range(18):
        start = -90 + n * 20
        a.arc(44, 44, 30, start, start + 13, QI if n < 11 else DIM, 2, .85 if n < 11 else .35)
    if refined:
        for rad, start, end in [(22, 40, 123), (24, 179, 233), (21, 260, 302)]:
            a.arc(44, 44, rad, start, end, QI, .7, .5)
        a.diamond(44, 6, 3, QI)
    return a


def generate():
    for revision in (1, 2):
        folder = ROOT / f"round-{revision}"
        folder.mkdir(parents=True, exist_ok=True)
        refined = revision == 2
        assets = {
            "mini-body": mini_body(refined), "mini-body-wounded": mini_body(refined, True),
            "quick-slot": slot(refined), "quick-slot-selected": slot(refined, True),
            "cast-gather": cast(refined, .25), "cast-form": cast(refined, .7),
            "cast-complete": cast(refined, 1, "complete"),
            "cast-interrupted": cast(refined, .7, "interrupted"),
            "target-health": target_hp(refined), "target-name": target_name(refined),
            "target-realm": target_realm(refined), "target-qi": target_qi(refined),
        }
        for name, art in assets.items():
            art.save(folder, name)
    print("已生成两轮独立预览 SVG；未写入生产资源目录。")


if __name__ == "__main__":
    generate()
