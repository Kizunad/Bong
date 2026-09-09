"""MiniBody 右侧状态条候选 B：连续柔边色带，使用生产支持的 SVG 图元。"""

from math import cos, pi, sin

from make_assets import Art, ROOT, mini_body_frame

TOP, BOTTOM = 18, 116
STATES = {"full": (1, 1), "normal": (.64, .82), "low": (.12, .18), "empty": (0, 0)}


def mix(a, b, t):
    values = [round(int(a[i:i + 2], 16) * (1 - t) + int(b[i:i + 2], 16) * t)
              for i in (1, 3, 5)]
    return "#" + "".join(f"{value:02x}" for value in values)


def radius(y):
    return 3.45 * max(0, sin(pi * (y - TOP) / (BOTTOM - TOP))) ** .19


def center(y):
    return 8 + .24 * sin(2 * pi * (y - TOP) / (BOTTOM - TOP))


def band(art, start, left, right, color, opacity=1):
    samples = [start + (BOTTOM - start) * n / 16 for n in range(17)]
    points = [(center(y) + radius(y) * left, y) for y in samples]
    points += [(center(y) + radius(y) * right, y) for y in reversed(samples)]
    art.poly(points, color, opacity)


def meter(kind, amount):
    a = Art(16, 128)
    base, light = ("#4e8e7c", "#b3cbbc") if kind == "qi" else ("#b38c55", "#e0c497")
    # 暗部保留整个容量的轮廓；连续色带的高度才表示当前值。
    band(a, TOP, -1.25, 1.25, "#111a17", .85)
    band(a, TOP, -1.12, 1.12, "#5e695a", .68)
    band(a, TOP, -1, 1, "#1b2922")
    band(a, TOP, -.75, .75, "#26372d")
    band(a, TOP, -.99, -.8, "#778876", .36)
    band(a, TOP, .78, .98, "#0e1713", .8)
    amount = max(0, min(1, amount))
    surface = BOTTOM - amount * (BOTTOM - TOP)
    if amount > 0:
        for n in range(11):
            u = (n + .5) / 11
            strength = .34 + .66 * max(0, cos((u - .43) * pi)) ** 2
            color = mix("#263b2d", base, strength)
            if abs(u - .36) < .13:
                color = mix(color, light, .35)
            band(a, surface, -1 + 2 * n / 11, -1 + 2 * (n + 1) / 11, color)
        if kind == "qi":
            # 小而连续的反光线强调液态，不额外引入颗粒或发光边框。
            for n in range(24):
                y1 = surface + 1 + max(0, BOTTOM - surface - 3) * n / 24
                y2 = surface + 1 + max(0, BOTTOM - surface - 3) * (n + 1) / 24
                x1 = center(y1) - radius(y1) * .4 + .25 * sin(y1 * .19)
                x2 = center(y2) - radius(y2) * .4 + .25 * sin(y2 * .19)
                a.line(x1, y1, x2, y2, light, .22, .44)
        else:
            # 纵向纹理让体力读作纤维束，保持与真元相同的计量边界。
            for x in (-.62, -.08, .52):
                band(a, surface, x, x + .06, "#6e5238", .8)
        if 0 < amount < 1:
            a.ellipse(center(surface), surface, radius(surface), .65, light, .85)
    a.line(5.5, 122, 10.5, 122, "#a7ac96", .65, .55)
    return a


def generate():
    folder = ROOT / "round-2-meters"
    folder.mkdir(parents=True, exist_ok=True)
    mini_body_frame(True).save(folder, "body-frame")
    for state, (qi, stamina) in STATES.items():
        meter("qi", qi).save(folder, f"qi-{state}")
        meter("stamina", stamina).save(folder, f"stamina-{state}")
    print(f"SVG 状态条预览: {folder}")


if __name__ == "__main__":
    generate()
