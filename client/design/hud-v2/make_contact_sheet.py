"""拼接生产 mesh 的两轮预览，PNG 输出到显式指定的目录。"""

from pathlib import Path
import argparse
import shutil
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
FONT = "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc"


def build(renders, output):
    sheet = Image.new("RGB", (1560, 1620), "#101512")
    pen = ImageDraw.Draw(sheet)

    def text(x, y, value, size=18, color="#d6cab0"):
        pen.text((x, y), value, font=ImageFont.truetype(FONT, size), fill=color)

    def asset(round_no, name, x, y, size):
        p = Image.open(renders / f"round-{round_no}" / f"{name}.png").convert("RGBA")
        p = p.resize(size, Image.Resampling.LANCZOS)
        sheet.paste(p, (x, y), p)

    text(52, 30, "末法残土 / HUD 视觉提案", 34)
    text(54, 85, "骨刻、残纸、淡墨人体。用不同轮廓表达信息，不再给所有内容套同一个矩形。", 19, "#999d8f")
    text(54, 120, "这是生产解析器生成的 mesh 离屏预览；尚未接入 Minecraft，文字为排版示意。", 17, "#999d8f")
    text(432, 176, "ROUND 1 / 初稿", 23)
    text(1046, 176, "ROUND 2 / 待人工看图", 23)

    rows = [
        (222, 280, "01 / 体感", "健康 / 左臂受伤", "人体线索，局部伤痕，细刻真元与体力"),
        (514, 174, "02 / 器槽", "普通 / 当前选中", "选中靠骨白缺口与底部刻片识别"),
        (700, 160, "03 / 起势", "25% / 70%", "残环随进度收拢，中心留空"),
        (872, 160, "04 / 收势", "完成 / 打断", "完成合印，打断错开并断裂"),
        (1044, 150, "05 / 感知 A", "生命 / 名称", "脉息刻度 / 窄幅题签，分别授权"),
        (1206, 178, "06 / 感知 B", "境界 / 真元", "层叠印记 / 气息环，分别授权"),
    ]
    for index, (y, h, title, subtitle, note) in enumerate(rows):
        pen.rounded_rectangle((36, y, 1524, y + h), radius=12, fill="#1a201c")
        text(56, y + 24, title, 24)
        text(56, y + 62, subtitle, 18, "#b4b5a7")
        # 两轮必须使用同一尺寸、底色、状态和排列。
        for round_no, x in [(1, 346), (2, 954)]:
            if index == 0:
                asset(round_no, "mini-body", x + 54, y + 18, (168, 222))
                asset(round_no, "mini-body-wounded", x + 280, y + 18, (168, 222))
            elif index == 1:
                for n in range(5):
                    asset(round_no, "quick-slot-selected" if n == 2 else "quick-slot", x + 27 + 88 * n, y + 23, (78, 85))
                    text(x + 42 + 88 * n, y + 112, str(n + 1), 15, "#8f9a8b")
            elif index in (2, 3):
                names = ("cast-gather", "cast-form") if index == 2 else ("cast-complete", "cast-interrupted")
                for n, name in enumerate(names):
                    asset(round_no, name, x + 61 + 226 * n, y + 13, (124, 124))
            elif index == 4:
                asset(round_no, "target-health", x + 12, y + 36, (224, 44))
                asset(round_no, "target-name", x + 254, y + 34, (224, 52))
                text(x + 326, y + 51, "某修士", 18)
            else:
                asset(round_no, "target-realm", x + 55, y + 11, (151, 119))
                text(x + 109, y + 57, "固元", 17)
                asset(round_no, "target-qi", x + 292, y + 15, (115, 115))
                text(x + 335, y + 63, "真元", 15)
        text(347, y + h - 27, note, 15, "#8e9b8d")

    text(54, 1412, "实检 / 两轮 24 个 SVG 均由 NanoSvgParser → SvgTessellator 通过；最大 352 三角形，无越界。", 18)
    text(54, 1446, "差分自证 / 注入不支持的 filter，生产解析器确实拒绝。此检查只证明兼容性，不证明美观。", 17, "#999d8f")
    text(54, 1480, "待定 / 人体辨识度、槽位厚重程度、残环位置与动效；实际 GUI 小尺寸和游戏遮挡需终轮验证。", 17, "#c3a676")
    text(54, 1514, "感知四件套 / 仅独立原型。默认全部锁定，不猜测功法 ID，不在客户端解锁服务器未授权的信息。", 17, "#999d8f")
    text(54, 1555, "新分支 feat/r7-hud-art-direction · Round 2 人工闸门 · 未提交", 15, "#728275")
    output.mkdir(parents=True, exist_ok=True)
    sheet.save(output / "contact-sheet.png")
    shutil.copyfile(renders / "asset-check.csv", output / "asset-check.csv")
    print(output / "contact-sheet.png")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("renders", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    build(args.renders, args.output)
