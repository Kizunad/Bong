#!/usr/bin/env python3
"""Convert a single-person pose video into Bong Emotecraft v3 animation JSON.

P0 intentionally keeps the MediaPipe/OpenCV boundary thin.  The converter core
is pure numpy/math so tests can pin coordinate, bend, and emitter contracts
without requiring video dependencies in every dev environment.
"""

from __future__ import annotations

import argparse
import math
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Sequence

import numpy as np

from anim_common import ANGLE_AXES, VALID_PARTS, build_doc, resolve_output_path, write_json


LM_NOSE = 0
LM_LEFT_EYE = 3
LM_RIGHT_EYE = 6
LM_MOUTH_LEFT = 9
LM_MOUTH_RIGHT = 10
LM_LEFT_SHOULDER = 11
LM_RIGHT_SHOULDER = 12
LM_LEFT_ELBOW = 13
LM_RIGHT_ELBOW = 14
LM_LEFT_WRIST = 15
LM_RIGHT_WRIST = 16
LM_LEFT_HIP = 23
LM_RIGHT_HIP = 24
LM_LEFT_KNEE = 25
LM_RIGHT_KNEE = 26
LM_LEFT_ANKLE = 27
LM_RIGHT_ANKLE = 28

VIDEO_EXTENSIONS = frozenset({".mp4", ".mov"})
PART_ORDER = ("body", "torso", "head", "leftArm", "rightArm", "leftLeg", "rightLeg")
AXIS_ORDER = ("x", "y", "z", "pitch", "yaw", "roll", "bend", "axis")
LINEAR_AXES = frozenset({"x", "y", "z"})


@dataclass(frozen=True)
class LandmarkFrame:
    """MediaPipe landmark pair sampled for one output animation tick."""

    tick: int
    world_landmarks: Sequence[object] | None
    image_landmarks: Sequence[object] | None
    source_frame_index: int | None = None
    source_time_seconds: float | None = None


@dataclass(frozen=True)
class BatchResult:
    """Summary of a batch conversion run."""

    converted: list[Path]
    skipped: list[Path]


class VideoPoser:
    """Sample MediaPipe Pose landmarks from a video at Minecraft tick cadence."""

    def __init__(self, *, fps: int = 20, model_complexity: int = 2) -> None:
        """Create a MediaPipe-backed sampler with validated target FPS."""
        if fps <= 0:
            raise ValueError("fps must be > 0")
        self.fps = fps
        try:
            import cv2  # type: ignore
            import mediapipe as mp  # type: ignore
        except ImportError as exc:
            raise RuntimeError(
                "video2emotecraft requires mediapipe and opencv-contrib-python; "
                "install client/tools/requirements-video2anim.txt"
            ) from exc
        self._cv2 = cv2
        self._pose = mp.solutions.pose.Pose(
            static_image_mode=False,
            model_complexity=model_complexity,
            enable_segmentation=False,
            smooth_landmarks=True,
        )

    def sample(self, video_path: Path) -> list[LandmarkFrame]:
        """Read a video file and return sampled world/image landmarks."""
        cap = self._cv2.VideoCapture(str(video_path))
        if not cap.isOpened():
            raise ValueError(f"cannot open video: {video_path}")

        source_fps = float(cap.get(self._cv2.CAP_PROP_FPS))
        if not math.isfinite(source_fps) or source_fps <= 0:
            source_fps = float(self.fps)
        sampler = FrameSampler(source_fps=source_fps, target_fps=self.fps)
        frames: list[LandmarkFrame] = []
        frame_index = 0
        try:
            while True:
                ok, frame_bgr = cap.read()
                if not ok:
                    break
                tick = sampler.tick_for_frame(frame_index)
                if tick is None:
                    frame_index += 1
                    continue
                frame_rgb = self._cv2.cvtColor(frame_bgr, self._cv2.COLOR_BGR2RGB)
                result = self._pose.process(frame_rgb)
                world = (
                    result.pose_world_landmarks.landmark
                    if result.pose_world_landmarks is not None
                    else None
                )
                image = (
                    result.pose_landmarks.landmark
                    if result.pose_landmarks is not None
                    else None
                )
                frames.append(
                    LandmarkFrame(
                        tick=tick,
                        world_landmarks=world,
                        image_landmarks=image,
                        source_frame_index=frame_index,
                        source_time_seconds=frame_index / source_fps,
                    )
                )
                frame_index += 1
        finally:
            cap.release()
        return frames


class PoseToEmotecraft:
    """Map MediaPipe landmarks to Bong PlayerAnimator pose tables."""

    def __init__(self, *, translate: bool = False, body_scale: float = 1.0) -> None:
        """Create a landmark converter with optional body translation output."""
        self.translate = translate
        self.body_scale = body_scale

    @staticmethod
    def _p(point: object) -> np.ndarray:
        """MediaPipe world X-right/Y-up/Z-camera -> PlayerAnimator X-right/Y-up/Z-front."""
        x, y, z = _xyz(point)
        return np.array([-x, y, -z], dtype=float)

    def convert_frames(
        self,
        frames: Iterable[LandmarkFrame],
        *,
        smooth: bool = True,
        loop: bool = False,
    ) -> dict[int, dict]:
        """Convert sampled landmark frames into an anim_common pose table."""
        pose_table: dict[int, dict] = {}
        for frame in frames:
            if frame.world_landmarks is None:
                continue
            pose_table[frame.tick] = self.frame_to_pose(
                frame.world_landmarks,
                frame.image_landmarks,
            )

        if smooth:
            _smooth_pose_table(pose_table)
        if loop and pose_table:
            first_tick = min(pose_table)
            last_tick = max(pose_table)
            pose_table[last_tick] = _merge_boundary_pose(pose_table[last_tick], pose_table[first_tick])
        return pose_table

    def frame_to_pose(
        self,
        world_landmarks: Sequence[object],
        image_landmarks: Sequence[object] | None = None,
    ) -> dict:
        """Convert one valid MediaPipe landmark frame into part axes."""
        points = {idx: self._p(world_landmarks[idx]) for idx in _REQUIRED_WORLD_LANDMARKS}

        shoulder_mid = _mid(points[LM_LEFT_SHOULDER], points[LM_RIGHT_SHOULDER])
        hip_mid = _mid(points[LM_LEFT_HIP], points[LM_RIGHT_HIP])
        torso_vec = shoulder_mid - hip_mid

        pose = {
            "body": self._body_translation(image_landmarks),
            "torso": _vector_pose(torso_vec, np.array([0.0, 1.0, 0.0])),
            "head": self._head_pose(points, torso_vec),
            "leftArm": self._arm_pose(
                points[LM_LEFT_SHOULDER],
                points[LM_LEFT_ELBOW],
                points[LM_LEFT_WRIST],
                reference=np.array([1.0, 0.0, 0.0]),
            ),
            "rightArm": self._arm_pose(
                points[LM_RIGHT_SHOULDER],
                points[LM_RIGHT_ELBOW],
                points[LM_RIGHT_WRIST],
                reference=np.array([-1.0, 0.0, 0.0]),
            ),
            "leftLeg": self._leg_pose(
                points[LM_LEFT_HIP],
                points[LM_LEFT_KNEE],
                points[LM_LEFT_ANKLE],
            ),
            "rightLeg": self._leg_pose(
                points[LM_RIGHT_HIP],
                points[LM_RIGHT_KNEE],
                points[LM_RIGHT_ANKLE],
            ),
        }
        return {part: axes for part, axes in pose.items() if axes}

    def _body_translation(self, image_landmarks: Sequence[object] | None) -> dict:
        """Return body xyz translation from normalized hip landmarks."""
        if not self.translate or image_landmarks is None:
            return {}
        hip = _mid(_vec_from_any(image_landmarks[LM_LEFT_HIP]), _vec_from_any(image_landmarks[LM_RIGHT_HIP]))
        return {
            "x": round((hip[0] - 0.5) * self.body_scale, 4),
            "y": round((0.5 - hip[1]) * self.body_scale, 4),
            "z": 0.0,
        }

    def _head_pose(self, points: dict[int, np.ndarray], torso_vec: np.ndarray) -> dict:
        """Estimate head pitch/yaw/roll from face landmark direction."""
        face_center = _mid(
            _mid(points[LM_LEFT_EYE], points[LM_RIGHT_EYE]),
            _mid(points[LM_MOUTH_LEFT], points[LM_MOUTH_RIGHT]),
        )
        face_vec = points[LM_NOSE] - face_center
        if np.linalg.norm(face_vec) < 1e-9:
            return {"pitch": 0.0, "yaw": 0.0, "roll": 0.0}
        return _vector_pose(face_vec, np.array([0.0, 0.0, -1.0]))

    def _arm_pose(
        self,
        shoulder: np.ndarray,
        elbow: np.ndarray,
        wrist: np.ndarray,
        *,
        reference: np.ndarray,
    ) -> dict:
        """Estimate upper-arm rotation and lower-arm bend axes."""
        upper = elbow - shoulder
        lower = wrist - elbow
        axes = _vector_pose(upper, reference)
        bend, axis = self._decompose_bend(upper, lower, is_leg=False)
        axes["bend"] = math.degrees(bend)
        axes["axis"] = math.degrees(axis)
        return axes

    def _leg_pose(self, hip: np.ndarray, knee: np.ndarray, ankle: np.ndarray) -> dict:
        """Estimate thigh rotation and knee bend axes."""
        thigh = knee - hip
        calf = ankle - knee
        axes = _vector_pose(thigh, np.array([0.0, -1.0, 0.0]))
        bend, axis = self._decompose_bend(thigh, calf, is_leg=True)
        axes["bend"] = math.degrees(bend)
        axes["axis"] = math.degrees(axis)
        return axes

    @staticmethod
    def _decompose_bend(
        parent_vec: np.ndarray,
        child_vec: np.ndarray,
        *,
        is_leg: bool = False,
    ) -> tuple[float, float]:
        """Return bend magnitude and bend axis in radians."""
        parent = _normalize(parent_vec)
        child = _normalize(child_vec)
        bend = math.acos(_clamp(float(np.dot(parent, child)), -1.0, 1.0))
        if is_leg:
            return bend, 0.0
        if bend < 1e-6:
            return 0.0, math.pi
        # PlayerAnimator arms usually need axis=180° to fold toward player front.
        # The rough generator pins that convention; P2 can refine per-frame axis.
        return bend, math.pi

    def build_doc(
        self,
        pose_table: dict[int, dict],
        *,
        name: str,
        loop: bool = False,
    ) -> dict:
        """Build an Emotecraft v3 JSON document from a pose table."""
        if not pose_table:
            raise ValueError("no valid pose frames found")
        end_tick = max(pose_table)
        return build_doc(
            pose_table,
            name=name,
            description=f"video2emotecraft generated rough animation: {name}",
            end_tick=end_tick,
            stop_tick=end_tick + 2,
            is_loop=loop,
        )


def _xyz(point: object) -> tuple[float, float, float]:
    """Read x/y/z from a MediaPipe-like object or tuple."""
    if hasattr(point, "x") and hasattr(point, "y"):
        return float(point.x), float(point.y), float(getattr(point, "z", 0.0))
    if isinstance(point, np.ndarray):
        return float(point[0]), float(point[1]), float(point[2] if len(point) > 2 else 0.0)
    return float(point[0]), float(point[1]), float(point[2] if len(point) > 2 else 0.0)  # type: ignore[index]


def sample_frame_indices(frame_count: int, source_fps: float, target_fps: int) -> list[int]:
    """Return source frame indices sampled at target tick cadence."""
    if frame_count < 0:
        raise ValueError("frame_count must be >= 0")
    sampler = FrameSampler(source_fps=source_fps, target_fps=target_fps)
    return [
        frame_index
        for frame_index in range(frame_count)
        if sampler.tick_for_frame(frame_index) is not None
    ]


def select_key_pose_table(
    pose_table: dict[int, dict],
    *,
    angle_threshold_degrees: float = 5.0,
) -> dict[int, dict]:
    """Return sparse key poses whose angle deltas exceed a threshold."""
    if not math.isfinite(angle_threshold_degrees) or angle_threshold_degrees < 0:
        raise ValueError("angle_threshold_degrees must be >= 0 and finite")
    ticks = sorted(pose_table)
    if len(ticks) <= 2:
        return {tick: pose_table[tick] for tick in ticks}

    selected: list[int] = [ticks[0]]
    last_kept = ticks[0]
    for tick in ticks[1:-1]:
        delta = _max_angle_delta(pose_table[last_kept], pose_table[tick])
        if delta >= angle_threshold_degrees:
            selected.append(tick)
            last_kept = tick
    if selected[-1] != ticks[-1]:
        selected.append(ticks[-1])
    return {tick: pose_table[tick] for tick in selected}


def write_gen_script(
    pose_table: dict[int, dict],
    *,
    name: str,
    out_path: Path | None = None,
    source_info: dict[int, tuple[int | None, float | None]] | None = None,
    loop: bool = False,
    angle_threshold_degrees: float = 5.0,
) -> Path:
    """Write a hand-editable gen_NAME.py script from a dense pose table."""
    if not pose_table:
        raise ValueError("no valid pose frames found")
    out_path = out_path or Path(__file__).resolve().parent / f"gen_{_safe_gen_name(name)}.py"
    key_pose_table = select_key_pose_table(
        pose_table,
        angle_threshold_degrees=angle_threshold_degrees,
    )
    rounded_pose_table = _round_pose_table_for_gen(key_pose_table)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        _build_gen_script_source(
            rounded_pose_table,
            name=name,
            source_info=source_info or {},
            loop=loop,
        )
    )
    return out_path


def frame_source_info(frames: Iterable[LandmarkFrame]) -> dict[int, tuple[int | None, float | None]]:
    """Return source frame/time metadata keyed by output tick."""
    return {
        frame.tick: (frame.source_frame_index, frame.source_time_seconds)
        for frame in frames
        if frame.world_landmarks is not None
    }


def convert_video_file(
    input_video: Path,
    *,
    output_name: str,
    fps: int = 20,
    model_complexity: int = 2,
    translate: bool = False,
    smooth: bool = True,
    loop: bool = False,
    preview: bool = False,
    output_path: Path | None = None,
    export_gen_name: str | None = None,
    angle_threshold_degrees: float = 5.0,
) -> Path:
    """Convert one video into either JSON or a hand-editable gen script."""
    poser = VideoPoser(fps=fps, model_complexity=model_complexity)
    frames = poser.sample(input_video)
    converter = PoseToEmotecraft(translate=translate)
    pose_table = converter.convert_frames(frames, smooth=smooth, loop=loop)
    if export_gen_name is not None:
        gen_path = write_gen_script(
            pose_table,
            name=export_gen_name,
            source_info=frame_source_info(frames),
            loop=loop,
            angle_threshold_degrees=angle_threshold_degrees,
        )
        keyframes = len(
            select_key_pose_table(
                pose_table,
                angle_threshold_degrees=angle_threshold_degrees,
            )
        )
        print(f"wrote {gen_path} keyframes={keyframes}")
        return gen_path

    doc = converter.build_doc(pose_table, name=output_name, loop=loop)
    out_path = write_json(doc, output_path or resolve_output_path(output_name))
    print(f"wrote {out_path} frames={len(pose_table)} moves={len(doc['emote']['moves'])}")
    if preview:
        _run_preview(out_path)
    return out_path


def batch_convert(
    input_dir: Path,
    output_dir: Path,
    *,
    fps: int = 20,
    model_complexity: int = 2,
    translate: bool = False,
    smooth: bool = True,
    loop: bool = False,
    preview: bool = False,
    convert_one: Callable[[Path, Path], Path] | None = None,
) -> BatchResult:
    """Convert all supported videos in a directory, skipping existing JSON."""
    if not input_dir.is_dir():
        raise ValueError(f"input_dir must be a directory: {input_dir}")
    output_dir.mkdir(parents=True, exist_ok=True)

    converted: list[Path] = []
    skipped: list[Path] = []
    for video_path in iter_video_files(input_dir):
        out_path = output_dir / f"{video_path.stem}.json"
        if out_path.exists() and out_path.stat().st_size > 0:
            skipped.append(out_path)
            continue
        if convert_one is None:
            written = convert_video_file(
                video_path,
                output_name=video_path.stem,
                fps=fps,
                model_complexity=model_complexity,
                translate=translate,
                smooth=smooth,
                loop=loop,
                preview=preview,
                output_path=out_path,
            )
        else:
            written = convert_one(video_path, out_path)
        converted.append(written)
    return BatchResult(converted=converted, skipped=skipped)


def iter_video_files(input_dir: Path) -> list[Path]:
    """Return supported video files in stable batch order."""
    return sorted(
        path
        for path in input_dir.iterdir()
        if path.is_file() and path.suffix.lower() in VIDEO_EXTENSIONS
    )


class FrameSampler:
    """Streaming source-frame sampler for a target animation tick cadence."""

    def __init__(self, *, source_fps: float, target_fps: int) -> None:
        """Create a streaming sampler for source fps to target fps conversion."""
        if not math.isfinite(source_fps) or source_fps <= 0:
            raise ValueError("source_fps must be > 0")
        if target_fps <= 0:
            raise ValueError("target_fps must be > 0")
        self.source_fps = source_fps
        self.target_fps = target_fps
        self._last_tick: int | None = None

    def tick_for_frame(self, frame_index: int) -> int | None:
        """Return the time-preserving output tick, or None for duplicate ticks."""
        frame_time = frame_index / self.source_fps
        tick = math.floor(frame_time * self.target_fps + 1e-9)
        if tick == self._last_tick:
            return None
        self._last_tick = tick
        return tick


def _vec_from_any(point: object) -> np.ndarray:
    """Return a numpy vector from any supported landmark point."""
    return np.array(_xyz(point), dtype=float)


def _mid(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """Return the midpoint between two vectors."""
    return (a + b) * 0.5


def _normalize(vec: np.ndarray) -> np.ndarray:
    """Return a unit vector, or zero for near-zero input."""
    norm = float(np.linalg.norm(vec))
    if norm < 1e-9:
        return np.zeros(3, dtype=float)
    return vec / norm


def _clamp(value: float, lo: float, hi: float) -> float:
    """Clamp a scalar into an inclusive range."""
    return max(lo, min(hi, value))


def _vector_pose(vector: np.ndarray, reference: np.ndarray) -> dict[str, float]:
    """Return pitch/yaw/roll degrees rotating reference onto vector."""
    matrix = _rotation_between(reference, vector)
    pitch, yaw, roll = _matrix_to_euler_xyz(matrix)
    return {
        "pitch": round(math.degrees(pitch), 4),
        "yaw": round(math.degrees(yaw), 4),
        "roll": round(math.degrees(roll), 4),
    }


def _rotation_between(reference: np.ndarray, target: np.ndarray) -> np.ndarray:
    """Build a rotation matrix that aligns reference to target."""
    ref = _normalize(reference)
    tgt = _normalize(target)
    if np.linalg.norm(ref) < 1e-9 or np.linalg.norm(tgt) < 1e-9:
        return np.eye(3)
    dot = _clamp(float(np.dot(ref, tgt)), -1.0, 1.0)
    if dot > 1.0 - 1e-9:
        return np.eye(3)
    if dot < -1.0 + 1e-9:
        axis = _normalize(np.cross(ref, np.array([1.0, 0.0, 0.0])))
        if np.linalg.norm(axis) < 1e-9:
            axis = _normalize(np.cross(ref, np.array([0.0, 1.0, 0.0])))
        return _axis_angle(axis, math.pi)
    axis = _normalize(np.cross(ref, tgt))
    return _axis_angle(axis, math.acos(dot))


def _axis_angle(axis: np.ndarray, angle: float) -> np.ndarray:
    """Build a 3x3 rotation matrix from axis-angle input."""
    x, y, z = axis
    c = math.cos(angle)
    s = math.sin(angle)
    one_c = 1.0 - c
    return np.array(
        [
            [c + x * x * one_c, x * y * one_c - z * s, x * z * one_c + y * s],
            [y * x * one_c + z * s, c + y * y * one_c, y * z * one_c - x * s],
            [z * x * one_c - y * s, z * y * one_c + x * s, c + z * z * one_c],
        ],
        dtype=float,
    )


def _matrix_to_euler_xyz(matrix: np.ndarray) -> tuple[float, float, float]:
    """Extract approximate PlayerAnimator pitch/yaw/roll radians."""
    sy = _clamp(float(matrix[0, 2]), -1.0, 1.0)
    yaw = math.asin(sy)
    if abs(sy) < 0.999999:
        pitch = math.atan2(-float(matrix[1, 2]), float(matrix[2, 2]))
        roll = math.atan2(-float(matrix[0, 1]), float(matrix[0, 0]))
    else:
        pitch = math.atan2(float(matrix[2, 1]), float(matrix[1, 1]))
        roll = 0.0
    return pitch, yaw, roll


def _smooth_pose_table(pose_table: dict[int, dict]) -> None:
    """Unwrap angular axes in-place to avoid ±180° discontinuities."""
    if len(pose_table) < 2:
        return
    for part in VALID_PARTS:
        axes = sorted({axis for pose in pose_table.values() for axis in pose.get(part, {})})
        for axis in axes:
            if axis not in {"pitch", "yaw", "roll", "bend", "axis"}:
                continue
            ticks = [tick for tick in sorted(pose_table) if axis in pose_table[tick].get(part, {})]
            if len(ticks) < 2:
                continue
            values = [float(pose_table[tick][part][axis]) for tick in ticks]
            smoothed = smooth_angle_degrees(values)
            for tick, value in zip(ticks, smoothed):
                pose_table[tick][part][axis] = round(value, 4)


def smooth_angle_degrees(values: Sequence[float]) -> list[float]:
    """Unwrap a degree sequence across the ±180° boundary."""
    radians = np.radians(np.array(values, dtype=float))
    return [float(v) for v in np.degrees(np.unwrap(radians))]


def _merge_boundary_pose(last_pose: dict, first_pose: dict) -> dict:
    """Copy first-pose axes onto the loop boundary pose."""
    merged = {part: dict(axes) if isinstance(axes, dict) else axes for part, axes in last_pose.items()}
    for part, axes in first_pose.items():
        if isinstance(axes, dict):
            merged[part] = dict(axes)
        else:
            merged[part] = axes
    return merged


def _max_angle_delta(previous_pose: dict, current_pose: dict) -> float:
    """Return the largest angular delta, preserving any linear-axis change."""
    largest = 0.0
    for part in set(previous_pose) | set(current_pose):
        previous_axes = previous_pose.get(part, {})
        current_axes = current_pose.get(part, {})
        if not isinstance(previous_axes, dict) or not isinstance(current_axes, dict):
            continue
        for axis in ANGLE_AXES:
            has_previous = axis in previous_axes
            has_current = axis in current_axes
            if has_previous != has_current:
                return math.inf
            if has_previous and has_current:
                largest = max(largest, abs(float(current_axes[axis]) - float(previous_axes[axis])))
        for axis in LINEAR_AXES:
            has_previous = axis in previous_axes
            has_current = axis in current_axes
            if has_previous != has_current:
                return math.inf
            if has_previous and has_current and float(current_axes[axis]) != float(previous_axes[axis]):
                return math.inf
    return largest


def _round_pose_table_for_gen(pose_table: dict[int, dict]) -> dict[int, dict]:
    """Round a pose table for hand-editable generator output."""
    rounded: dict[int, dict] = {}
    for tick, pose in pose_table.items():
        rounded_pose: dict = {}
        for part in PART_ORDER:
            axes = pose.get(part)
            if not isinstance(axes, dict):
                continue
            rounded_pose[part] = {
                axis: _round_gen_axis(axis, axes[axis])
                for axis in AXIS_ORDER
                if axis in axes
            }
        rounded[tick] = rounded_pose
    return rounded


def _round_gen_axis(axis: str, value: object) -> float:
    """Round angles to 0.5° and linear axes to 4 decimals."""
    numeric = float(value)
    if axis in ANGLE_AXES:
        return round(round(numeric * 2.0) / 2.0, 1)
    return round(numeric, 4)


def _build_gen_script_source(
    pose_table: dict[int, dict],
    *,
    name: str,
    source_info: dict[int, tuple[int | None, float | None]],
    loop: bool,
) -> str:
    """Build Python source for a generated gen_NAME.py script."""
    end_tick = max(pose_table)
    lines = [
        "#!/usr/bin/env python3",
        '"""Generated by video2emotecraft --export-gen.',
        "",
        "Edit POSE, then run this file to emit the rough animation JSON.",
        '"""',
        "",
        "from __future__ import annotations",
        "",
        "from anim_common import emit_json",
        "",
        "",
        "POSE = {",
    ]
    for tick in sorted(pose_table):
        frame_index, frame_time = source_info.get(tick, (None, None))
        lines.append(f"    # {_format_source_comment(frame_index, frame_time)}")
        lines.append(f"    {tick}: {{")
        pose = pose_table[tick]
        for part in PART_ORDER:
            axes = pose.get(part)
            if not axes:
                continue
            axis_items = ", ".join(f"{axis!r}: {axes[axis]!r}" for axis in AXIS_ORDER if axis in axes)
            lines.append(f"        {part!r}: {{{axis_items}}},")
        lines.append("    },")
    lines.extend(
        [
            "}",
            "",
            "",
            'if __name__ == "__main__":',
            "    emit_json(",
            "        POSE,",
            f"        name={name!r},",
            f"        description={'video2emotecraft exported rough animation: ' + name!r},",
            f"        end_tick={end_tick},",
            f"        stop_tick={end_tick + 2},",
            f"        is_loop={loop!r},",
            "    )",
            "",
        ]
    )
    return "\n".join(lines)


def _format_source_comment(frame_index: int | None, frame_time: float | None) -> str:
    """Format source frame metadata for generated script comments."""
    if frame_index is None or frame_time is None:
        return "source frame unknown"
    return f"source frame {frame_index} @ {frame_time:.3f}s"


def _safe_gen_name(name: str) -> str:
    """Return a filename-safe suffix for gen_NAME.py."""
    safe = re.sub(r"[^\w]+", "_", name, flags=re.UNICODE).strip("_")
    if not safe:
        raise ValueError("export-gen name must contain a filename-safe character")
    if safe[0].isdigit():
        safe = f"anim_{safe}"
    return safe


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse CLI arguments for the video converter."""
    if argv and argv[0] == "batch":
        parser = argparse.ArgumentParser(description="Batch convert pose videos to Bong Emotecraft v3 JSON")
        parser.add_argument("command", choices=("batch",))
        parser.add_argument("input_dir", type=Path)
        parser.add_argument("-o", "--output", required=True, type=Path, help="output directory")
        _add_common_cli_args(parser)
        return parser.parse_args(argv)

    parser = argparse.ArgumentParser(description="Convert pose video to Bong Emotecraft v3 JSON")
    parser.add_argument("input_video", type=Path)
    parser.add_argument("-o", "--output", help="animation name without .json")
    parser.add_argument(
        "--export-gen",
        metavar="NAME",
        help="write client/tools/gen_NAME.py instead of direct JSON",
    )
    parser.add_argument(
        "--key-threshold",
        type=_non_negative_float,
        default=5.0,
        help="minimum angular delta in degrees for --export-gen keyframes",
    )
    _add_common_cli_args(parser)
    args = parser.parse_args(argv)
    if args.output is None and args.export_gen is None:
        parser.error("-o/--output is required unless --export-gen NAME is used")
    if args.output is not None and args.export_gen is not None and args.output != args.export_gen:
        parser.error("--export-gen NAME and -o/--output NAME must match when both are set")
    args.command = "convert"
    return args


def _add_common_cli_args(parser: argparse.ArgumentParser) -> None:
    """Add flags shared by single-file and batch conversion."""
    parser.add_argument("--fps", type=_positive_int, default=20)
    parser.add_argument("--complexity", type=int, default=2, choices=(0, 1, 2))
    parser.add_argument("--translate", action="store_true")
    parser.add_argument("--no-smooth", action="store_true")
    parser.add_argument("--loop", action="store_true")
    parser.add_argument("--preview", action="store_true")


def _positive_int(value: str) -> int:
    """Parse a strictly positive integer for argparse."""
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be > 0")
    return parsed


def _non_negative_float(value: str) -> float:
    """Parse a non-negative float for argparse."""
    parsed = float(value)
    if not math.isfinite(parsed) or parsed < 0:
        raise argparse.ArgumentTypeError("must be >= 0 and finite")
    return parsed


def main(argv: Sequence[str] | None = None) -> int:
    """Run the video-to-animation CLI pipeline."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.command == "batch":
        result = batch_convert(
            args.input_dir,
            args.output,
            fps=args.fps,
            model_complexity=args.complexity,
            translate=args.translate,
            smooth=not args.no_smooth,
            loop=args.loop,
            preview=args.preview,
        )
        print(f"batch converted={len(result.converted)} skipped={len(result.skipped)}")
        return 0

    output_name = args.export_gen or args.output
    convert_video_file(
        args.input_video,
        output_name=output_name,
        fps=args.fps,
        model_complexity=args.complexity,
        translate=args.translate,
        smooth=not args.no_smooth,
        loop=args.loop,
        preview=args.preview,
        export_gen_name=args.export_gen,
        angle_threshold_degrees=args.key_threshold,
    )
    return 0


def _run_preview(out_path: Path) -> None:
    """Render a preview grid for the generated animation JSON."""
    preview_dir = Path(tempfile.gettempdir()) / "video2anim_preview" / out_path.stem
    preview_dir.mkdir(parents=True, exist_ok=True)
    try:
        from render_animation import render_grid

        grid_path = render_grid(out_path, preview_dir)
        print(f"preview: {grid_path}")
    except Exception as exc:
        print(f"warning: preview failed: {exc}", file=sys.stderr)


_REQUIRED_WORLD_LANDMARKS = {
    LM_NOSE,
    LM_LEFT_EYE,
    LM_RIGHT_EYE,
    LM_MOUTH_LEFT,
    LM_MOUTH_RIGHT,
    LM_LEFT_SHOULDER,
    LM_RIGHT_SHOULDER,
    LM_LEFT_ELBOW,
    LM_RIGHT_ELBOW,
    LM_LEFT_WRIST,
    LM_RIGHT_WRIST,
    LM_LEFT_HIP,
    LM_RIGHT_HIP,
    LM_LEFT_KNEE,
    LM_RIGHT_KNEE,
    LM_LEFT_ANKLE,
    LM_RIGHT_ANKLE,
}


if __name__ == "__main__":
    raise SystemExit(main())
