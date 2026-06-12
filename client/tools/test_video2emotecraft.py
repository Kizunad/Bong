from __future__ import annotations

import argparse
import math
import subprocess
import sys
from itertools import pairwise
from pathlib import Path

import numpy as np
import pytest

from anim_common import VALID_PARTS
from video2emotecraft import (
    apply_reference_calibration,
    batch_convert,
    FrameSampler,
    infer_keyframe_easing,
    iter_video_files,
    LandmarkFrame,
    parse_frame_range,
    PoseToEmotecraft,
    parse_args,
    sample_frame_indices,
    select_key_pose_table,
    smooth_angle_degrees,
    _safe_gen_name,
    write_gen_script,
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
    assert pose["leftArm"]["bend"] == 0.0 and pose["rightArm"]["bend"] == 0.0, (
        f"expected straight T-pose arms to have zero bend, actual left/right "
        f"{pose['leftArm']['bend']}/{pose['rightArm']['bend']}"
    )


def test_public_pose_reports_left_arm_bend_90_degrees() -> None:
    """Verify arm bend through the public frame_to_pose API."""
    landmarks = canonical_t_pose()
    landmarks[15] = Lm(-0.7, 1.7, 0.0)

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
    landmarks[27] = Lm(-0.65, -1.05, 0.0)

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


def test_build_doc_rejects_empty_pose_table() -> None:
    """Verify all-dropped video input surfaces a clear error."""
    with pytest.raises(ValueError, match="no valid pose frames"):
        PoseToEmotecraft().build_doc({}, name="empty_from_video")


def test_build_doc_loop_preserves_linear_axes() -> None:
    """Verify looped multi-frame docs keep xyz linear units."""
    doc = PoseToEmotecraft().build_doc(
        {
            0: {"body": {"x": 0.25}, "rightArm": {"pitch": 0.0}},
            1: {"body": {"x": 0.5}, "rightArm": {"pitch": 45.0}},
            2: {"body": {"x": 0.25}, "rightArm": {"pitch": 0.0}},
        },
        name="loop_from_video",
        loop=True,
    )

    assert doc["emote"]["isLoop"] is True, (
        f"expected loop flag to propagate into Emotecraft doc, actual {doc['emote']['isLoop']}"
    )
    body_move = next(move for move in doc["emote"]["moves"] if move.get("body", {}).get("x") == 0.25)
    assert body_move["body"]["x"] == 0.25, (
        f"expected body.x to stay linear meters rather than radians, actual {body_move['body']['x']}"
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


def test_antiparallel_torso_rotation_is_finite() -> None:
    """Verify upside-down torso input takes the anti-parallel branch safely."""
    landmarks = canonical_t_pose()
    landmarks[11] = Lm(-0.2, -0.2, 0.0)
    landmarks[12] = Lm(0.2, -0.2, 0.0)
    landmarks[23] = Lm(-0.15, 1.2, 0.0)
    landmarks[24] = Lm(0.15, 1.2, 0.0)

    pose = PoseToEmotecraft().frame_to_pose(landmarks)

    assert all(math.isfinite(pose["torso"][axis]) for axis in ("pitch", "yaw", "roll")), (
        f"expected finite torso axes for anti-parallel vector, actual {pose['torso']}"
    )


def test_near_gimbal_arm_pose_is_finite_through_public_api() -> None:
    """Verify sideways-to-front arm input remains finite through frame_to_pose."""
    landmarks = canonical_t_pose()
    landmarks[13] = Lm(-0.2, 1.2, -1.0)
    landmarks[15] = Lm(-0.2, 1.2, -2.0)

    pose = PoseToEmotecraft().frame_to_pose(landmarks)

    assert all(math.isfinite(pose["leftArm"][axis]) for axis in ("pitch", "yaw", "roll")), (
        f"expected finite public leftArm axes for near-gimbal input, actual {pose['leftArm']}"
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


def test_savgol_smoothing_reduces_high_frequency_noise() -> None:
    """Verify Savitzky-Golay smoothing reduces synthetic angular jitter."""
    baseline = np.linspace(0.0, 20.0, 21)
    noise = np.array([2.0 if index % 2 == 0 else -2.0 for index in range(21)])
    noisy = baseline + noise

    smoothed = np.array(smooth_angle_degrees(noisy.tolist(), window=5))

    before = float(np.std(noisy - baseline))
    after = float(np.std(smoothed - baseline))
    assert after < before * 0.5, (
        f"expected Savitzky-Golay smoothing to cut high-frequency jitter by >50%, "
        f"before={before:.4f} after={after:.4f}"
    )


def test_smooth_window_zero_keeps_unwrap_only() -> None:
    """Verify --smooth-window 0 disables denoising while preserving unwrap behavior."""
    values = [170.0, 179.0, -179.0, -170.0]

    smoothed = smooth_angle_degrees(values, window=0)
    deltas = [abs(b - a) for a, b in pairwise(smoothed)]

    assert max(deltas) < 180.0, (
        f"expected window=0 to keep unwrap-only smoothing, actual deltas={deltas}"
    )


def test_smooth_window_rejects_even_values() -> None:
    """Verify direct smoothing rejects Savitzky-Golay windows scipy cannot apply."""
    with pytest.raises(ValueError, match="odd integer"):
        smooth_angle_degrees([0.0, 1.0, 2.0, 3.0], window=4)


def test_smooth_window_rejects_less_than_minimum_odd() -> None:
    """Verify direct smoothing rejects the off-by-one minimum window boundary."""
    with pytest.raises(ValueError, match="odd integer"):
        smooth_angle_degrees([0.0, 1.0, 2.0], window=1)


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


def test_low_fps_source_ticks_preserve_duration() -> None:
    """Verify low-fps input maps source time onto sparse target ticks."""
    sampler = FrameSampler(source_fps=10.0, target_fps=20)
    ticks = [sampler.tick_for_frame(frame_index) for frame_index in range(10)]

    assert ticks == list(range(0, 20, 2)), (
        "expected 10fps source frames to land on even 20tps ticks so one second remains one second, "
        f"actual {ticks}"
    )


@pytest.mark.parametrize(
    ("frame_count", "expected"),
    [
        (0, []),
        (1, [0]),
    ],
)
def test_sample_frame_indices_boundary_cases(frame_count: int, expected: list[int]) -> None:
    """Verify empty and one-frame sampling boundaries."""
    indices = sample_frame_indices(frame_count=frame_count, source_fps=30.0, target_fps=20)

    assert indices == expected, (
        f"expected {expected} because empty/single-frame inputs must preserve boundaries, actual {indices}"
    )


def test_sample_frame_indices_rejects_negative_frame_count() -> None:
    """Verify negative frame counts surface the explicit validation error."""
    with pytest.raises(ValueError, match="frame_count must be >= 0"):
        sample_frame_indices(frame_count=-1, source_fps=30.0, target_fps=20)


@pytest.mark.parametrize("fps", ["0", "-1"])
def test_non_positive_fps_is_rejected(fps: str, capsys: pytest.CaptureFixture[str]) -> None:
    """Verify CLI rejects zero FPS before video sampling."""
    with pytest.raises(SystemExit) as exc_info:
        parse_args(["input.mp4", "-o", "bad", "--fps", fps])

    stderr = capsys.readouterr().err
    assert exc_info.value.code != 0, (
        f"expected non-zero SystemExit because --fps={fps} is invalid, actual {exc_info.value.code}"
    )
    assert "must be > 0" in stderr, (
        f"expected argparse stderr to include the FPS validation message, actual stderr={stderr!r}"
    )


@pytest.mark.parametrize("threshold", ["-0.5", "nan", "inf"])
def test_invalid_key_threshold_is_rejected(
    threshold: str,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Verify CLI rejects impossible export-gen keyframe thresholds before conversion."""
    with pytest.raises(SystemExit) as exc_info:
        parse_args(["input.mp4", "--export-gen", "unit", "--key-threshold", threshold])

    stderr = capsys.readouterr().err
    assert exc_info.value.code != 0, (
        f"expected non-zero SystemExit because --key-threshold={threshold} is invalid, "
        f"actual {exc_info.value.code}"
    )
    assert "must be >= 0" in stderr, (
        f"expected argparse stderr to include the threshold validation message, actual stderr={stderr!r}"
    )


@pytest.mark.parametrize("window", ["2", "4"])
def test_invalid_smooth_window_is_rejected(
    window: str,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Verify CLI rejects invalid Savitzky-Golay windows before conversion."""
    with pytest.raises(SystemExit) as exc_info:
        parse_args(["input.mp4", "-o", "bad", "--smooth-window", window])

    stderr = capsys.readouterr().err
    assert exc_info.value.code != 0, (
        f"expected non-zero SystemExit because --smooth-window={window} is invalid, actual {exc_info.value.code}"
    )
    assert "odd integer" in stderr, (
        f"expected argparse stderr to explain smooth-window oddness, actual stderr={stderr!r}"
    )


def test_calibrate_range_cli_parses_inclusive_ticks() -> None:
    """Verify CLI parses T-pose calibration frame ranges into inclusive tick tuples."""
    args = parse_args(["input.mp4", "-o", "unit", "--calibrate", "0-40", "--smooth-window", "0"])

    assert args.calibrate == (0, 40), (
        f"expected --calibrate 0-40 to parse as inclusive ticks, actual {args.calibrate}"
    )
    assert args.smooth_window == 0, (
        f"expected --smooth-window 0 to keep unwrap-only smoothing, actual {args.smooth_window}"
    )


@pytest.mark.parametrize("value", ["bad", "4-", "5-1"])
def test_parse_frame_range_rejects_invalid_ranges(value: str) -> None:
    """Verify calibration range parser rejects malformed or reversed input."""
    with pytest.raises(argparse.ArgumentTypeError):
        parse_frame_range(value)


@pytest.mark.parametrize(
    ("source_fps", "target_fps", "message"),
    [
        (0.0, 20, "source_fps must be > 0"),
        (-1.0, 20, "source_fps must be > 0"),
        (float("nan"), 20, "source_fps must be > 0"),
        (30.0, 0, "target_fps must be > 0"),
    ],
)
def test_frame_sampler_rejects_invalid_fps(
    source_fps: float,
    target_fps: int,
    message: str,
) -> None:
    """Verify direct FrameSampler guard branches are pinned."""
    with pytest.raises(ValueError, match=message):
        FrameSampler(source_fps=source_fps, target_fps=target_fps)


def test_export_gen_script_is_valid_python_with_source_comments(tmp_path) -> None:
    """Verify exported gen script compiles and carries source frame comments."""
    out_path = tmp_path / "gen_unit_export.py"
    written = write_gen_script(
        {
            0: {"rightArm": {"pitch": 0.0, "bend": 0.0, "axis": 180.0}},
            10: {"rightArm": {"pitch": 40.0, "bend": 20.0, "axis": 180.0}},
        },
        name="unit_export",
        out_path=out_path,
        source_info={0: (0, 0.0), 10: (20, 1.0)},
    )

    source = written.read_text()
    compile(source, str(written), "exec")
    assert "# source frame 0 @ 0.000s" in source, (
        f"expected generated script to document original source frame, actual source={source}"
    )
    assert "name='unit_export'" in source, (
        f"expected generated script to emit the requested animation name, actual source={source}"
    )


def test_export_gen_script_runs_with_emit_json_contract(tmp_path) -> None:
    """Verify exported gen script can run as a standalone Python file."""
    script_path = tmp_path / "gen_run_unit.py"
    write_gen_script(
        {
            0: {"rightArm": {"pitch": 0.0, "bend": 0.0, "axis": 180.0}},
            5: {"rightArm": {"pitch": 30.0, "bend": 15.0, "axis": 180.0}},
        },
        name="run_unit",
        out_path=script_path,
    )
    (tmp_path / "anim_common.py").write_text(
        "from pathlib import Path\n"
        "def emit_json(pose_table, **kwargs):\n"
        "    Path('emitted.txt').write_text(f\"{kwargs['name']}:{len(pose_table)}\")\n"
        "    return Path('emitted.txt')\n"
    )

    subprocess.run([sys.executable, str(script_path)], cwd=tmp_path, check=True)

    emitted = (tmp_path / "emitted.txt").read_text()
    assert emitted == "run_unit:2", (
        f"expected generated script to call emit_json with pose table and name, actual {emitted!r}"
    )


def test_export_gen_accepts_unicode_filename_suffix() -> None:
    """Verify non-ASCII animation names do not crash export-gen filename sanitization."""
    safe_name = _safe_gen_name("动画")

    assert safe_name == "动画", (
        f"expected Chinese export-gen names to remain usable on local filesystems, actual {safe_name!r}"
    )


def test_export_gen_rejects_empty_filename_suffix() -> None:
    """Verify export-gen still rejects names with no usable filename characters."""
    with pytest.raises(ValueError, match="filename-safe character"):
        _safe_gen_name("!!!")


def test_export_gen_prefixes_digit_filename_suffix() -> None:
    """Verify digit-leading export-gen names are prefixed into valid script stems."""
    safe_name = _safe_gen_name("1name")

    assert safe_name == "anim_1name", (
        f"expected digit-leading export-gen names to gain anim_ prefix, actual {safe_name!r}"
    )


def test_keyframe_filter_keeps_sparse_angle_changes() -> None:
    """Verify export-gen keeps key frames rather than every sampled frame."""
    pose_table = {tick: {"rightArm": {"pitch": float(tick)}} for tick in range(60)}

    selected = select_key_pose_table(pose_table, angle_threshold_degrees=5.0)

    expected_ticks = list(range(0, 60, 5)) + [59]
    assert list(selected) == expected_ticks, (
        "expected sparse keyframes every 5° plus final boundary because threshold is 5°, "
        f"actual ticks={list(selected)}"
    )
    assert 0 in selected and 59 in selected, (
        f"expected export-gen to preserve boundary keyframes, actual ticks={sorted(selected)}"
    )


def test_keyframe_filter_preserves_translate_only_changes() -> None:
    """Verify --translate poses are not collapsed when only body xyz changes."""
    pose_table = {tick: {"body": {"x": tick * 0.1}} for tick in range(4)}

    selected = select_key_pose_table(pose_table, angle_threshold_degrees=5.0)

    assert list(selected) == [0, 1, 2, 3], (
        f"expected translate-only frames to be preserved because body.x is observable, actual {list(selected)}"
    )


@pytest.mark.parametrize("threshold", [-1.0, float("nan"), float("inf")])
def test_keyframe_filter_rejects_invalid_thresholds(threshold: float) -> None:
    """Verify direct keyframe selection rejects non-finite and negative thresholds."""
    with pytest.raises(ValueError, match="angle_threshold_degrees must be >= 0"):
        select_key_pose_table({0: {"rightArm": {"pitch": 0.0}}}, angle_threshold_degrees=threshold)


def test_easing_inference_marks_acceleration() -> None:
    """Verify increasing angular velocity is annotated as EASEINQUAD for gen export."""
    pose_table = {
        0: {"rightArm": {"pitch": 0.0}},
        5: {"rightArm": {"pitch": 2.0}},
        10: {"rightArm": {"pitch": 8.0}},
    }

    easing = infer_keyframe_easing(pose_table)

    assert easing[10] == "EASEINQUAD", (
        f"expected rising angular velocity to infer EASEINQUAD, actual {easing}"
    )


def test_easing_inference_empty_pose_returns_empty_mapping() -> None:
    """Verify empty keyframe input returns an empty easing map."""
    easing = infer_keyframe_easing({})

    assert easing == {}, (
        f"expected empty easing map because no keyframes exist, actual {easing}"
    )


def test_easing_inference_short_series_defaults_linear() -> None:
    """Verify fewer than three ticks cannot infer acceleration and stays linear."""
    pose_table = {
        0: {"rightArm": {"pitch": 1.0}},
        5: {"rightArm": {"pitch": 3.0}},
    }

    easing = infer_keyframe_easing(pose_table)

    assert easing == {0: "linear", 5: "linear"}, (
        f"expected linear defaults for fewer than 3 ticks, actual {easing}"
    )


def test_easing_inference_marks_deceleration() -> None:
    """Verify decreasing angular velocity is annotated as EASEOUTQUAD for gen export."""
    pose_table = {
        0: {"rightArm": {"pitch": 0.0}},
        5: {"rightArm": {"pitch": 8.0}},
        10: {"rightArm": {"pitch": 10.0}},
    }

    easing = infer_keyframe_easing(pose_table)

    assert easing[10] == "EASEOUTQUAD", (
        f"expected falling angular velocity to infer EASEOUTQUAD, actual {easing}"
    )


def test_easing_inference_keeps_uniform_velocity_linear() -> None:
    """Verify uniform angular velocity remains linear rather than overfitted."""
    pose_table = {
        0: {"rightArm": {"pitch": 0.0}},
        5: {"rightArm": {"pitch": 5.0}},
        10: {"rightArm": {"pitch": 10.0}},
    }

    easing = infer_keyframe_easing(pose_table)

    assert easing[10] == "linear", (
        f"expected constant angular velocity to stay linear, actual {easing}"
    )


def test_export_gen_script_includes_inferred_easing(tmp_path) -> None:
    """Verify export-gen writes inferred easing markers into hand-editable scripts."""
    out_path = tmp_path / "gen_easing.py"
    write_gen_script(
        {
            0: {"rightArm": {"pitch": 0.0}},
            5: {"rightArm": {"pitch": 8.0}},
            10: {"rightArm": {"pitch": 10.0}},
        },
        name="easing",
        out_path=out_path,
        angle_threshold_degrees=0.0,
    )

    source = out_path.read_text()
    assert "'easing': 'EASEOUTQUAD'" in source, (
        f"expected decelerating rough pose to mark EASEOUTQUAD in gen script, actual source={source}"
    )


def test_reference_calibration_removes_t_pose_offset() -> None:
    """Verify calibration subtracts average reference rotation from all frames."""
    pose_table = {
        0: {"rightArm": {"pitch": 10.0, "yaw": -5.0, "bend": 2.0, "axis": 180.0}},
        1: {"rightArm": {"pitch": 10.0, "yaw": -5.0, "bend": 2.0, "axis": 180.0}},
        2: {"rightArm": {"pitch": 14.0, "yaw": -2.0, "bend": 5.0, "axis": 180.0}},
    }

    calibrated = apply_reference_calibration(pose_table, (0, 1))

    assert abs(calibrated[0]["rightArm"]["pitch"]) < 1.0, (
        f"expected reference pitch residual <1° after calibration, actual {calibrated[0]}"
    )
    assert abs(calibrated[0]["rightArm"]["yaw"]) < 1.0, (
        f"expected reference yaw residual <1° after calibration, actual {calibrated[0]}"
    )
    assert calibrated[2]["rightArm"]["pitch"] == 4.0, (
        f"expected action frames to preserve motion relative to reference, actual {calibrated[2]}"
    )
    assert calibrated[2]["rightArm"]["axis"] == 180.0, (
        f"expected bend axis to remain a plane marker rather than calibration offset, actual {calibrated[2]}"
    )


def test_reference_calibration_uses_circular_mean_for_angle_offsets() -> None:
    """Verify calibration averages reference angles across the ±180° seam correctly."""
    pose_table = {
        0: {"head": {"yaw": 179.0}},
        1: {"head": {"yaw": -179.0}},
        2: {"head": {"yaw": 181.0}},
    }

    calibrated = apply_reference_calibration(pose_table, (0, 1))

    assert abs(calibrated[0]["head"]["yaw"]) <= 1.1, (
        f"expected 179°/-179° reference mean near 180° rather than 0°, actual {calibrated[0]}"
    )
    assert abs(calibrated[1]["head"]["yaw"]) <= 1.1, (
        f"expected seam-crossing reference to stay near zero residual, actual {calibrated[1]}"
    )


def test_reference_calibration_rejects_reversed_range() -> None:
    """Verify calibration rejects reversed reference ranges before sampling."""
    with pytest.raises(ValueError, match="start must be <= end"):
        apply_reference_calibration({0: {"rightArm": {"pitch": 10.0}}}, (3, 1))


def test_reference_calibration_rejects_empty_reference_range() -> None:
    """Verify calibration fails clearly when the requested T-pose range has no frames."""
    with pytest.raises(ValueError, match="did not match"):
        apply_reference_calibration({10: {"rightArm": {"pitch": 10.0}}}, (0, 1))


def test_export_gen_rounds_angles_to_half_degree(tmp_path) -> None:
    """Verify exported angle values are rounded to hand-editable 0.5° precision."""
    out_path = tmp_path / "gen_rounding.py"
    write_gen_script(
        {
            0: {"rightArm": {"pitch": 12.24, "bend": 90.26, "axis": 179.76}},
            1: {"rightArm": {"pitch": 12.76, "bend": 89.74, "axis": 180.24}},
        },
        name="rounding",
        out_path=out_path,
        angle_threshold_degrees=0.0,
    )

    source = out_path.read_text()
    assert "'pitch': 12.0" in source and "'bend': 90.5" in source and "'axis': 180.0" in source, (
        f"expected generated angle literals on the 0.5° grid, actual source={source}"
    )


def test_batch_convert_is_idempotent(tmp_path) -> None:
    """Verify batch mode skips JSON files already generated in a prior run."""
    input_dir = tmp_path / "videos"
    output_dir = tmp_path / "out"
    input_dir.mkdir()
    output_dir.mkdir()
    (input_dir / "a.mp4").write_bytes(b"fake")
    (input_dir / "b.mov").write_bytes(b"fake")
    (input_dir / "ignore.txt").write_text("not a video")
    calls: list[str] = []

    def fake_convert(video_path, out_path) -> Path:
        calls.append(video_path.name)
        out_path.write_text("{}")
        return out_path

    first = batch_convert(input_dir, output_dir, convert_one=fake_convert)
    second = batch_convert(input_dir, output_dir, convert_one=fake_convert)

    assert calls == ["a.mp4", "b.mov"], (
        f"expected first batch run to convert each supported video once, actual calls={calls}"
    )
    assert [path.name for path in first.converted] == ["a.json", "b.json"], (
        f"expected first run outputs for supported videos only, actual {first.converted}"
    )
    assert second.converted == [], (
        f"expected second run to skip all existing outputs, actual converted={second.converted}"
    )
    assert [path.name for path in second.skipped] == ["a.json", "b.json"], (
        f"expected second run to report skipped outputs, actual skipped={second.skipped}"
    )


def test_batch_convert_retries_zero_byte_outputs(tmp_path) -> None:
    """Verify batch mode does not treat a crash-left empty JSON as a completed output."""
    input_dir = tmp_path / "videos"
    output_dir = tmp_path / "out"
    input_dir.mkdir()
    output_dir.mkdir()
    (input_dir / "a.mp4").write_bytes(b"fake")
    (output_dir / "a.json").write_bytes(b"")
    calls: list[str] = []

    def fake_convert(video_path, out_path) -> Path:
        calls.append(video_path.name)
        out_path.write_text('{"ok":true}')
        return out_path

    result = batch_convert(input_dir, output_dir, convert_one=fake_convert)

    assert calls == ["a.mp4"], (
        f"expected zero-byte output to be retried because prior conversion was incomplete, actual calls={calls}"
    )
    assert [path.name for path in result.converted] == ["a.json"], (
        f"expected retried output to be reported as converted, actual converted={result.converted}"
    )
    assert result.skipped == [], (
        f"expected zero-byte output not to be reported as skipped, actual skipped={result.skipped}"
    )


def test_iter_video_files_uses_supported_extensions_only(tmp_path) -> None:
    """Verify batch discovery is stable and extension-filtered."""
    (tmp_path / "b.MOV").write_bytes(b"fake")
    (tmp_path / "a.mp4").write_bytes(b"fake")
    (tmp_path / "c.avi").write_bytes(b"fake")

    videos = iter_video_files(tmp_path)

    assert [path.name for path in videos] == ["a.mp4", "b.MOV"], (
        f"expected only .mp4/.mov files in sorted order, actual {[path.name for path in videos]}"
    )


def canonical_t_pose() -> list[Lm]:
    """Build a 33-landmark neutral pose compatible with MediaPipe indices."""
    landmarks = [Lm(0.0, 0.0, 0.0) for _ in range(33)]

    # Values are pre-transform MediaPipe world-like coordinates.  After conversion:
    # shoulders/arms lie on +/-X, torso points +Y, legs point -Y, face points -Z.
    landmarks[0] = Lm(0.0, 1.4, 1.0)
    landmarks[3] = Lm(0.1, 1.45, 0.0)
    landmarks[6] = Lm(-0.1, 1.45, 0.0)
    landmarks[9] = Lm(0.08, 1.35, 0.0)
    landmarks[10] = Lm(-0.08, 1.35, 0.0)
    landmarks[11] = Lm(-0.2, 1.2, 0.0)
    landmarks[12] = Lm(0.2, 1.2, 0.0)
    landmarks[13] = Lm(-0.7, 1.2, 0.0)
    landmarks[14] = Lm(0.7, 1.2, 0.0)
    landmarks[15] = Lm(-1.1, 1.2, 0.0)
    landmarks[16] = Lm(1.1, 1.2, 0.0)
    landmarks[23] = Lm(-0.15, 0.2, 0.0)
    landmarks[24] = Lm(0.15, 0.2, 0.0)
    landmarks[25] = Lm(-0.15, -0.55, 0.0)
    landmarks[26] = Lm(0.15, -0.55, 0.0)
    landmarks[27] = Lm(-0.15, -1.2, 0.0)
    landmarks[28] = Lm(0.15, -1.2, 0.0)
    return landmarks
