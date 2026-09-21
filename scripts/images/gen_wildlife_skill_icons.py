"""绘制兽技的小尺寸剪影图标；无需联网，固定几何保证资源可重复生成。"""

from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "client/src/main/resources/assets/bong/textures/gui/skill/fauna"
SCALE = 4
INK = "#201b1b"
BONE = "#eedbbc"
GOLD = "#bb8645"
RED = "#a94432"
ASH = "#87818b"


class Emblem:
    def __init__(self):
        self.image = Image.new("RGBA", (64 * SCALE, 64 * SCALE))
        self.draw = ImageDraw.Draw(self.image)

    def shape(self, points, color):
        scaled = [(x * SCALE, y * SCALE) for x, y in points]
        self.draw.polygon(scaled, fill=color)
        self.draw.line(scaled + [scaled[0]], fill=INK, width=SCALE, joint="curve")

    def stroke(self, points, color, width=2):
        self.draw.line(
            [(x * SCALE, y * SCALE) for x, y in points],
            fill=color,
            width=width * SCALE,
            joint="curve",
        )

    def save(self, name):
        self.image.resize((64, 64), Image.Resampling.LANCZOS).save(OUTPUT / f"{name}.png")


def lion_pounce():
    art = Emblem()
    art.stroke([(5, 48), (17, 39), (32, 35), (50, 27)], GOLD, 3)
    art.shape([(12, 30), (9, 20), (16, 16), (22, 24), (32, 20), (40, 23),
               (41, 32), (30, 36), (23, 33), (17, 40), (8, 43), (9, 38), (15, 33)], GOLD)
    art.shape([(35, 19), (41, 12), (51, 15), (55, 24), (50, 34), (39, 35), (33, 27)], RED)
    art.shape([(41, 19), (49, 18), (53, 24), (58, 25), (56, 29), (45, 29), (40, 25)], BONE)
    art.shape([(35, 31), (42, 32), (47, 41), (56, 43), (55, 47), (43, 46), (36, 38)], GOLD)
    art.stroke([(47, 22), (50, 22)], INK)
    art.save("lion_pounce")


def lion_rend():
    art = Emblem()
    art.shape([(12, 17), (22, 10), (44, 13), (54, 24), (45, 30), (35, 24), (22, 28)], GOLD)
    art.shape([(18, 29), (25, 24), (28, 42)], BONE)
    art.shape([(41, 24), (48, 29), (35, 42)], BONE)
    art.shape([(13, 43), (26, 51), (43, 51), (52, 41), (45, 36), (33, 43), (21, 37)], GOLD)
    for offset in [0, 9, 18]:
        art.stroke([(18 + offset, 31), (13 + offset, 51), (9 + offset, 57)], RED)
    art.save("lion_rend")


def vulture_dive():
    art = Emblem()
    art.shape([(31, 47), (23, 35), (8, 28), (4, 10), (14, 17), (13, 8), (22, 21),
               (27, 30), (33, 34), (41, 24), (50, 11), (49, 22), (60, 13), (54, 33), (41, 39)], ASH)
    art.shape([(29, 39), (35, 37), (40, 44), (37, 52), (31, 49)], BONE)
    art.shape([(32, 48), (37, 50), (31, 58), (29, 52)], GOLD)
    art.stroke([(18, 35), (21, 46), (25, 51)], RED)
    art.stroke([(46, 40), (43, 49)], GOLD)
    art.save("vulture_dive")


def hoof(art, x, y, color):
    art.shape([(x, y), (x + 7, y - 2), (x + 10, y + 15), (x + 5, y + 22),
               (x - 6, y + 20), (x - 5, y + 15), (x + 2, y + 11)], color)
    art.shape([(x - 5, y + 16), (x + 9, y + 15), (x + 5, y + 22), (x - 6, y + 20)], ASH)


def horse_skills():
    art = Emblem()
    for x, y in [(14, 15), (46, 12), (31, 25)]:
        hoof(art, x, y, BONE)
    art.stroke([(5, 49), (17, 45), (23, 55), (34, 51), (40, 57), (58, 46)], GOLD, 3)
    art.save("horse_trample")

    art = Emblem()
    art.shape([(45, 10), (58, 17), (52, 32), (45, 40), (32, 35), (23, 27),
               (9, 26), (7, 19), (25, 18), (38, 26)], BONE)
    art.shape([(44, 30), (49, 36), (33, 49), (14, 43), (14, 37), (30, 39)], GOLD)
    art.shape([(7, 19), (15, 20), (14, 27), (5, 28), (4, 24)], ASH)
    art.shape([(14, 37), (20, 39), (17, 46), (8, 44), (8, 39)], ASH)
    art.stroke([(20, 8), (11, 11), (6, 15)], RED, 3)
    art.save("horse_kick")


if __name__ == "__main__":
    OUTPUT.mkdir(parents=True, exist_ok=True)
    lion_pounce()
    lion_rend()
    vulture_dive()
    horse_skills()
    print(f"已生成 5 张兽技图标：{OUTPUT}")
