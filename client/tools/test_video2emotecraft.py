from __future__ import annotations

import math

import numpy as np

from anim_common import VALID_PARTS
from video2emotecraft import LandmarkFrame, PoseToEmotecraft, smooth_angle_degrees


class Lm:
    def __init__(self, x: float, y: float, z: float = 0.0) -> None:
        self.x = x
        self.y = y
        self.z = z


def test_coordinate_transform_signs() -> None:
    converted = PoseToEmotecraft._p(Lm(1.0, -2.0, 3.0))

    assert converted.tolist() == [-1.0, 2.0, -3.0]


def test_t_pose_maps_to_near_zero_rotations() -> None:
    pose = PoseToEmotecraft().frame_to_pose(canonical_t_pose())

    for part in ("torso", "head", "leftArm", "rightArm", "leftLeg", "rightLeg"):
        assert part in pose
        for axis in ("pitch", "yaw", "roll"):
            assert abs(pose[part][axis]) < 1e-4, f"{part}.{axis}={pose[part][axis]}"


def test_arm_bend_decomposition_90_degrees() -> None:
    bend, axis = PoseToEmotecraft._decompose_bend(
        np.array([1.0, 0.0, 0.0]),
        np.array([0.0, 1.0, 0.0]),
        is_leg=False,
    )

    assert math.isclose(bend, math.pi / 2.0, abs_tol=1e-6)
    assert math.isclose(axis, math.pi, abs_tol=1e-6)


def test_leg_bend_decomposition_45_degrees() -> None:
    bend, axis = PoseToEmotecraft._decompose_bend(
        np.array([0.0, -1.0, 0.0]),
        np.array([math.sin(math.pi / 4.0), -math.cos(math.pi / 4.0), 0.0]),
        is_leg=True,
    )

    assert math.isclose(bend, math.pi / 4.0, abs_tol=1e-6)
    assert axis == 0.0


def test_output_json_uses_radians_and_degrees_false() -> None:
    converter = PoseToEmotecraft()
    doc = converter.build_doc(
        {
            0: {
                "rightArm": {"pitch": 90.0, "bend": 45.0, "axis": 180.0},
            }
        },
        name="unit_from_video",
    )

    assert doc["emote"]["degrees"] is False
    moves = doc["emote"]["moves"]
    pitch_move = next(move for move in moves if "pitch" in move["rightArm"])
    bend_move = next(move for move in moves if "bend" in move["rightArm"])
    assert math.isclose(pitch_move["rightArm"]["pitch"], math.pi / 2.0, abs_tol=1e-7)
    assert math.isclose(bend_move["rightArm"]["bend"], math.pi / 4.0, abs_tol=1e-7)


def test_pose_table_structure_and_valid_parts() -> None:
    frames = [LandmarkFrame(0, canonical_t_pose(), None)]
    pose_table = PoseToEmotecraft().convert_frames(frames)

    assert set(pose_table.keys()) == {0}
    for tick, pose in pose_table.items():
        assert isinstance(tick, int)
        assert set(pose).issubset(VALID_PARTS)


def test_missing_landmarks_frame_is_skipped() -> None:
    frames = [
        LandmarkFrame(0, None, None),
        LandmarkFrame(1, canonical_t_pose(), None),
    ]

    pose_table = PoseToEmotecraft().convert_frames(frames)

    assert set(pose_table.keys()) == {1}


def test_angle_continuity_unwraps_180_boundary() -> None:
    smoothed = smooth_angle_degrees([170.0, 179.0, -179.0, -170.0])
    deltas = [abs(b - a) for a, b in zip(smoothed, smoothed[1:])]

    assert max(deltas) < 180.0
    assert smoothed[-1] > 180.0


def test_loop_mode_closes_boundary_pose() -> None:
    landmarks0 = canonical_t_pose()
    landmarks1 = canonical_t_pose()
    landmarks1[15] = Lm(-0.4, -1.0, 0.0)
    frames = [
        LandmarkFrame(0, landmarks0, None),
        LandmarkFrame(1, landmarks1, None),
    ]

    pose_table = PoseToEmotecraft().convert_frames(frames, loop=True)

    assert pose_table[1] == pose_table[0]


def canonical_t_pose() -> list[Lm]:
    landmarks = [Lm(0.0, 0.0, 0.0) for _ in range(33)]

    # Values are pre-transform MediaPipe-like coordinates.  After _p():
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
