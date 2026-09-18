# Window Icons

`lock-keyhole.svg`, `lock-keyhole-open.svg`, `minus.svg`, `x.svg`, and `maximize-2.svg` are Lucide icons from
https://github.com/lucide-icons/lucide/tree/a79b2d131dab2bf20cb224bd0937b439a9c4fa99/icons.
Only the stroke color was changed. See `LICENSE.txt` for attribution.

`rotate-ccw.svg` and `panels-top-left.svg` use the same Lucide source and license,
retrieved from its `main` branch on 2026-09-13; only the stroke color was changed.

Minecraft draws transparent 48 x 48 PNG derivatives at 14 x 14 GUI units.
The SVG sources use paths and strokes outside the HUD mesh parser's supported subset.
Regenerate from the repository root with CairoSVG 2.9.0:

```bash
for icon in lock-keyhole lock-keyhole-open minus x maximize-2 rotate-ccw panels-top-left; do
  uv run --with cairosvg==2.9.0 cairosvg \
    "client/src/main/resources/assets/bong-client/svg/ui/$icon.svg" \
    -s 2 -o "client/src/main/resources/assets/bong-client/textures/gui/window/$icon.png"
done
```
