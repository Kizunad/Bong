"""生成 HUD 预览；只使用当前生产解析器支持的 SVG 图元。"""

from math import cos, hypot, radians, sin
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INK, BONE, DIM = "#171b19", "#d6cab0", "#6c7266"
GOLD, BLOOD = "#ba996b", "#b86654"


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


def generate():
    folder = ROOT / "round-2"
    folder.mkdir(parents=True, exist_ok=True)
    assets = {
        "quick-slot": slot(True),
        "quick-slot-selected": slot(True, True),
        "cast-complete": cast(True, 1, "complete"),
        "cast-interrupted": cast(True, .7, "interrupted"),
    }
    for name, art in assets.items():
        art.save(folder, name)
    print("已生成第二轮生产接线所需的四个 SVG 预览；未写入生产资源目录。")


if __name__ == "__main__":
    generate()
