#!/usr/bin/env python3
"""Convert a single-person pose video into Bong Emotecraft v3 animation JSON.

P0 intentionally keeps the MediaPipe/OpenCV boundary thin.  The converter core
is pure numpy/math so tests can pin coordinate, bend, and emitter contracts
without requiring video dependencies in every dev environment.
"""

from __future__ import annotations

import argparse
import math
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

import numpy as np

from anim_common import VALID_PARTS, build_doc, resolve_output_path, write_json


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


@dataclass(frozen=True)
class LandmarkFrame:
    """MediaPipe landmark pair sampled for one output animation tick."""

    tick: int
    world_landmarks: Sequence[object] | None
    image_landmarks: Sequence[object] | None


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
                frames.append(LandmarkFrame(tick=tick, world_landmarks=world, image_landmarks=image))
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
        tick = math.floor(frame_time * float(self.target_fps) + 1e-9)
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


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse CLI arguments for the video converter."""
    parser = argparse.ArgumentParser(description="Convert pose video to Bong Emotecraft v3 JSON")
    parser.add_argument("input_video", type=Path)
    parser.add_argument("-o", "--output", required=True, help="animation name without .json")
    parser.add_argument("--fps", type=_positive_int, default=20)
    parser.add_argument("--complexity", type=int, default=2, choices=(0, 1, 2))
    parser.add_argument("--translate", action="store_true")
    parser.add_argument("--no-smooth", action="store_true")
    parser.add_argument("--loop", action="store_true")
    parser.add_argument("--preview", action="store_true")
    return parser.parse_args(argv)


def _positive_int(value: str) -> int:
    """Parse a strictly positive integer for argparse."""
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be > 0")
    return parsed


def main(argv: Sequence[str] | None = None) -> int:
    """Run the video-to-animation CLI pipeline."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    poser = VideoPoser(fps=args.fps, model_complexity=args.complexity)
    frames = poser.sample(args.input_video)
    converter = PoseToEmotecraft(translate=args.translate)
    pose_table = converter.convert_frames(frames, smooth=not args.no_smooth, loop=args.loop)
    doc = converter.build_doc(pose_table, name=args.output, loop=args.loop)
    out_path = write_json(doc, resolve_output_path(args.output))
    print(f"wrote {out_path} frames={len(pose_table)} moves={len(doc['emote']['moves'])}")
    if args.preview:
        _run_preview(out_path)
    return 0


def _run_preview(out_path: Path) -> None:
    """Render a preview grid for the generated animation JSON."""
    preview_dir = Path(tempfile.mkdtemp(prefix=f"video2anim_{out_path.stem}_"))
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
