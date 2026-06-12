from __future__ import annotations

import math
from itertools import pairwise

import numpy as np
import pytest

from anim_common import VALID_PARTS
from video2emotecraft import (
    LandmarkFrame,
    PoseToEmotecraft,
    parse_args,
    sample_frame_indices,
    smooth_angle_degrees,
)


class Lm:
    """Tiny MediaPipe-like landmark object for tests."""

    def __init__(self, x: float, y: float, z: float = 0.0) -> None:
        """Store x/y/z coordinates with MediaPipe attribute names."""
        self.x = x
        self.y = y
        self.z = z


def test_body_translation_signs_through_public_pose() -> None:
    """Verify public pose output preserves body translation signs."""
    image_landmarks = [Lm(0.0, 0.0, 0.0) for _ in range(33)]
    image_landmarks[23] = Lm(0.75, 0.25, 0.0)
    image_landmarks[24] = Lm(0.75, 0.25, 0.0)

    pose = PoseToEmotecraft(translate=True, body_scale=2.0).frame_to_pose(
        canonical_t_pose(),
        image_landmarks,
    )

    assert pose["body"] == {"x": 0.5, "y": 0.5, "z": 0.0}, (
        "expected positive body x/y because public translation maps image hip "
        f"(0.75,0.25) away from screen center, actual {pose['body']}"
    )


def test_t_pose_maps_to_near_zero_rotations() -> None:
    """Verify canonical T-pose is the neutral rotation baseline."""
    pose = PoseToEmotecraft().frame_to_pose(canonical_t_pose())

    for part in ("torso", "head", "leftArm", "rightArm", "leftLeg", "rightLeg"):
        assert part in pose, f"expected {part} in pose because T-pose has all required landmarks"
        for axis in ("pitch", "yaw", "roll"):
            assert abs(pose[part][axis]) < 1e-4, (
                f"expected {part}.{axis}≈0 because canonical T-pose is the neutral "
                f"reference, actual {pose[part][axis]}"
            )


def test_public_pose_reports_left_arm_bend_90_degrees() -> None:
    """Verify arm bend through the public frame_to_pose API."""
    landmarks = canonical_t_pose()
    landmarks[15] = Lm(-0.7, -1.7, 0.0)

    pose = PoseToEmotecraft().frame_to_pose(landmarks)

    assert math.isclose(pose["leftArm"]["bend"], 90.0, abs_tol=1e-4), (
        "expected left arm bend≈90° because the wrist is orthogonal to the upper arm, "
        f"actual {pose['leftArm']['bend']}"
    )
    assert math.isclose(pose["leftArm"]["axis"], 180.0, abs_tol=1e-4), (
        "expected left arm axis≈180° because arm bends use the Bong forward-folding "
        f"convention, actual {pose['leftArm']['axis']}"
    )


def test_public_pose_reports_left_leg_bend_45_degrees() -> None:
    """Verify leg bend through the public frame_to_pose API."""
    landmarks = canonical_t_pose()
    landmarks[27] = Lm(-0.65, 1.05, 0.0)

    pose = PoseToEmotecraft().frame_to_pose(landmarks)

    assert math.isclose(pose["leftLeg"]["bend"], 45.0, abs_tol=0.01), (
        "expected left leg bend≈45° because ankle vector is 45° from thigh, "
        f"actual {pose['leftLeg']['bend']}"
    )
    assert math.isclose(pose["leftLeg"]["axis"], 0.0, abs_tol=1e-6), (
        f"expected leg bend axis≈0 because knee bend is planar, actual {pose['leftLeg']['axis']}"
    )


def test_output_json_uses_radians_and_degrees_false() -> None:
    """Verify generated Emotecraft JSON stores angle moves as radians."""
    converter = PoseToEmotecraft()
    doc = converter.build_doc(
        {
            0: {
                "rightArm": {"pitch": 90.0, "bend": 45.0, "axis": 180.0},
            }
        },
        name="unit_from_video",
    )

    assert doc["emote"]["degrees"] is False, (
        f"expected degrees=false because anim_common emits radians, actual {doc['emote']['degrees']}"
    )
    moves = doc["emote"]["moves"]
    pitch_move = next(move for move in moves if "pitch" in move["rightArm"])
    bend_move = next(move for move in moves if "bend" in move["rightArm"])
    assert math.isclose(pitch_move["rightArm"]["pitch"], math.pi / 2.0, abs_tol=1e-7), (
        "expected 90° pitch to serialize as π/2 radians, "
        f"actual {pitch_move['rightArm']['pitch']}"
    )
    assert math.isclose(bend_move["rightArm"]["bend"], math.pi / 4.0, abs_tol=1e-7), (
        "expected 45° bend to serialize as π/4 radians, "
        f"actual {bend_move['rightArm']['bend']}"
    )


def test_pose_table_structure_and_valid_parts() -> None:
    """Verify pose table keys and part names match anim_common contracts."""
    frames = [LandmarkFrame(0, canonical_t_pose(), None)]
    pose_table = PoseToEmotecraft().convert_frames(frames)

    assert set(pose_table.keys()) == {0}, (
        f"expected only tick 0 because one valid frame was provided, actual {set(pose_table.keys())}"
    )
    for tick, pose in pose_table.items():
        assert isinstance(tick, int), f"expected int tick because emit_json requires int keys, actual {tick!r}"
        assert set(pose).issubset(VALID_PARTS), (
            f"expected pose parts within VALID_PARTS because PlayerAnimator rejects unknown parts, "
            f"actual {set(pose) - VALID_PARTS}"
        )


def test_missing_landmarks_frame_is_skipped() -> None:
    """Verify frames with missing landmarks are skipped safely."""
    frames = [
        LandmarkFrame(0, None, None),
        LandmarkFrame(1, canonical_t_pose(), None),
    ]

    pose_table = PoseToEmotecraft().convert_frames(frames)

    assert set(pose_table.keys()) == {1}, (
        "expected only tick 1 because tick 0 landmarks are missing and must be skipped, "
        f"actual ticks={set(pose_table.keys())}"
    )


def test_angle_continuity_unwraps_180_boundary() -> None:
    """Verify angle unwrap removes discontinuity across ±180 degrees."""
    smoothed = smooth_angle_degrees([170.0, 179.0, -179.0, -170.0])
    deltas = [abs(b - a) for a, b in pairwise(smoothed)]

    assert max(deltas) < 180.0, (
        f"expected adjacent deltas <180° because unwrap should preserve continuity, actual {deltas}"
    )
    assert smoothed[-1] > 180.0, (
        f"expected final angle to unwrap beyond 180° instead of jumping negative, actual {smoothed[-1]}"
    )


def test_loop_mode_closes_boundary_pose() -> None:
    """Verify loop mode copies tick zero pose to the boundary tick."""
    landmarks0 = canonical_t_pose()
    landmarks1 = canonical_t_pose()
    landmarks1[15] = Lm(-0.4, -1.0, 0.0)
    frames = [
        LandmarkFrame(0, landmarks0, None),
        LandmarkFrame(1, landmarks1, None),
    ]

    pose_table = PoseToEmotecraft().convert_frames(frames, loop=True)

    assert pose_table[1] == pose_table[0], (
        f"expected loop mode to copy tick 0 to the boundary tick, actual boundary {pose_table[1]}"
    )


def test_sample_frame_indices_preserve_30_to_20_fps_duration() -> None:
    """Verify 30fps source sampling keeps one second at 20 ticks."""
    indices = sample_frame_indices(frame_count=30, source_fps=30.0, target_fps=20)

    assert len(indices) == 20, (
        f"expected 20 samples for one second of 30fps source at 20tps target, actual {len(indices)}"
    )
    assert indices[0] == 0, f"expected first source frame sampled at tick 0, actual {indices[0]}"


def test_sample_frame_indices_do_not_duplicate_low_fps_source() -> None:
    """Verify low-fps sources are sampled once per available source frame."""
    indices = sample_frame_indices(frame_count=10, source_fps=10.0, target_fps=20)

    assert indices == list(range(10)), (
        "expected every low-fps source frame sampled once because converter does not synthesize frames, "
        f"actual {indices}"
    )


def test_non_positive_fps_is_rejected() -> None:
    """Verify CLI rejects zero FPS before video sampling."""
    with pytest.raises(SystemExit):
        parse_args(["input.mp4", "-o", "bad", "--fps", "0"])


def canonical_t_pose() -> list[Lm]:
    """Build a 33-landmark neutral pose compatible with MediaPipe indices."""
    landmarks = [Lm(0.0, 0.0, 0.0) for _ in range(33)]

    # Values are pre-transform MediaPipe-like coordinates.  After conversion:
    # shoulders/arms lie on +/-X, torso points +Y, legs point -Y, face points -Z.
    landmarks[0] = Lm(0.0, -1.4, 1.0)
    landmarks[3] = Lm(0.1, -1.45, 0.0)
    landmarks[6] = Lm(-0.1, -1.45, 0.0)
    landmarks[9] = Lm(0.08, -1.35, 0.0)
    landmarks[10] = Lm(-0.08, -1.35, 0.0)
    landmarks[11] = Lm(-0.2, -1.2, 0.0)
    landmarks[12] = Lm(0.2, -1.2, 0.0)
    landmarks[13] = Lm(-0.7, -1.2, 0.0)
    landmarks[14] = Lm(0.7, -1.2, 0.0)
    landmarks[15] = Lm(-1.1, -1.2, 0.0)
    landmarks[16] = Lm(1.1, -1.2, 0.0)
    landmarks[23] = Lm(-0.15, -0.2, 0.0)
    landmarks[24] = Lm(0.15, -0.2, 0.0)
    landmarks[25] = Lm(-0.15, 0.55, 0.0)
    landmarks[26] = Lm(0.15, 0.55, 0.0)
    landmarks[27] = Lm(-0.15, 1.2, 0.0)
    landmarks[28] = Lm(0.15, 1.2, 0.0)
    return landmarks
