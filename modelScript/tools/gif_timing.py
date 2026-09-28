"""动画预览 GIF 的逐帧时间表：两个预览工具（creature_anim_frames / preview_player_anim）共用。

契约：一轮 GIF 的总时长 = 动画时长（按播放速度换算）+ end_hold_ms。

- 在 [0, length) 上均匀取 count 帧，每帧 frame_ms；循环动画到此为止，末帧 == 首帧不重复。
- 一次性动画若要在收势停一下（end_hold_ms > 0），再补一帧 t = length，这一帧只承担停留时长。
  不能给它也算一个 frame_ms：那样每段一次性动画都凭空多播一帧，hold 为 0 时也会多停。
- end_hold_ms 为 0 就不补这一帧（GIF 帧时长不能是 0）。
"""

from __future__ import annotations


def gif_schedule(length: float, count: int, frame_ms: int, looped: bool,
                 end_hold_ms: int) -> list[tuple[float, int]]:
    """返回 [(采样时刻, 这一帧显示多少毫秒)]；时刻与 length 同单位（秒或 tick 都行）。"""

    if count < 1:
        raise ValueError(f"至少要 1 帧，收到 {count}")
    if end_hold_ms < 0:
        raise ValueError(f"end_hold_ms 必须非负，收到 {end_hold_ms}")
    schedule = [(length * i / count, frame_ms) for i in range(count)]
    if not looped and end_hold_ms > 0:
        schedule.append((length, end_hold_ms))
    return schedule
