#!/usr/bin/env python3
"""
AV Experience Visualizer v2 — generates 4 HTML files with real SVG poses,
CSS-animated particles, sound mixer bars, and HUD mockups for each combat style.

Output: scripts/balance/av-{dugu,tuike,baomai,zhenfa}.html
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path
from textwrap import dedent

SCRIPT_DIR = Path(__file__).resolve().parent


# ─── Data Model ─────────────────────────────────────────────────────────────

@dataclass
class SoundLayer:
    sound_id: str
    pitch: float
    volume: float
    delay_ticks: int = 0


@dataclass
class HudComponent:
    name: str
    description: str
    hud_type: str  # charge_ring, bar, taint_indicator, timer, stack, array_layout, body_silhouette
    extra: dict = field(default_factory=dict)


@dataclass
class Skill:
    name_cn: str
    name_en: str
    pose_svg: str        # SVG markup for the pose
    particle_html: str   # CSS/SVG animated particles
    sound_layers: list[SoundLayer] = field(default_factory=list)
    hud: list[HudComponent] = field(default_factory=list)


@dataclass
class Style:
    name_cn: str
    name_en: str
    accent: str
    skills: list[Skill]


# ─── SVG Pose Helpers ───────────────────────────────────────────────────────

def mc_figure(
    body_color: str = "#5a5a5a",
    skin_color: str = "#c8a882",
    head_rot: float = 0,
    torso_rot: float = 0,
    left_arm_rot: float = 0,
    right_arm_rot: float = 0,
    left_leg_rot: float = 0,
    right_leg_rot: float = 0,
    left_arm_x: float = 0,
    left_arm_y: float = 0,
    right_arm_x: float = 0,
    right_arm_y: float = 0,
    extras: str = "",
    scale: float = 3.0,
) -> str:
    """Generate a Minecraft-style blocky humanoid SVG with rotatable limbs."""
    return f'''<svg viewBox="-30 -5 60 55" width="180" height="165" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <style>
      .skin {{ fill: {skin_color}; }}
      .body {{ fill: {body_color}; }}
      .outline {{ stroke: #222; stroke-width: 0.3; fill: none; }}
    </style>
  </defs>
  <!-- Head -->
  <g transform="rotate({head_rot}, 0, 4)">
    <rect class="skin" x="-4" y="0" width="8" height="8" rx="0.5"/>
    <rect x="-3" y="2.5" width="1.5" height="1" fill="#333" rx="0.3"/>
    <rect x="1.5" y="2.5" width="1.5" height="1" fill="#333" rx="0.3"/>
    <rect class="outline" x="-4" y="0" width="8" height="8" rx="0.5"/>
  </g>
  <!-- Torso -->
  <g transform="rotate({torso_rot}, 0, 8)">
    <rect class="body" x="-4" y="8" width="8" height="12" rx="0.3"/>
    <rect class="outline" x="-4" y="8" width="8" height="12" rx="0.3"/>
  </g>
  <!-- Left Arm -->
  <g transform="translate({-4 + left_arm_x}, {8 + left_arm_y}) rotate({left_arm_rot}, 2, 0)">
    <rect class="body" x="-4" y="0" width="4" height="12" rx="0.3"/>
    <rect class="skin" x="-4" y="9" width="4" height="3" rx="0.3"/>
    <rect class="outline" x="-4" y="0" width="4" height="12" rx="0.3"/>
  </g>
  <!-- Right Arm -->
  <g transform="translate({4 + right_arm_x}, {8 + right_arm_y}) rotate({right_arm_rot}, 2, 0)">
    <rect class="body" x="0" y="0" width="4" height="12" rx="0.3"/>
    <rect class="skin" x="0" y="9" width="4" height="3" rx="0.3"/>
    <rect class="outline" x="0" y="0" width="4" height="12" rx="0.3"/>
  </g>
  <!-- Left Leg -->
  <g transform="translate(-2, 20) rotate({left_leg_rot}, 2, 0)">
    <rect class="body" x="-2" y="0" width="4" height="12" rx="0.3"/>
    <rect class="outline" x="-2" y="0" width="4" height="12" rx="0.3"/>
  </g>
  <!-- Right Leg -->
  <g transform="translate(2, 20) rotate({right_leg_rot}, 2, 0)">
    <rect class="body" x="-2" y="0" width="4" height="12" rx="0.3"/>
    <rect class="outline" x="-2" y="0" width="4" height="12" rx="0.3"/>
  </g>
  {extras}
</svg>'''


# ─── Baomai (体修) Poses ────────────────────────────────────────────────────

def baomai_poses() -> list[str]:
    # 1) 崩拳 — forward punch
    beng = mc_figure(
        body_color="#8b6914", skin_color="#d4a86a",
        right_arm_rot=-80, left_arm_rot=20,
        left_leg_rot=-15, right_leg_rot=15,
        extras='''
        <circle cx="12" cy="14" r="3" fill="#c9a96e" opacity="0.5">
          <animate attributeName="r" values="3;4;3" dur="0.6s" repeatCount="indefinite"/>
        </circle>
        <line x1="8" y1="14" x2="18" y2="12" stroke="#c9a96e" stroke-width="0.5" opacity="0.6">
          <animate attributeName="opacity" values="0.6;1;0.6" dur="0.4s" repeatCount="indefinite"/>
        </line>'''
    )
    # 2) 全力一击 — arms up holding sphere
    full = mc_figure(
        body_color="#8b6914", skin_color="#d4a86a",
        left_arm_rot=-150, right_arm_rot=-150,
        left_arm_x=2, right_arm_x=-2,
        extras='''
        <circle cx="0" cy="-2" r="5" fill="none" stroke="#c9a96e" stroke-width="0.8" opacity="0.7">
          <animate attributeName="r" values="4;6;4" dur="1s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.5;1;0.5" dur="1s" repeatCount="indefinite"/>
        </circle>
        <circle cx="0" cy="-2" r="3" fill="#ffd700" opacity="0.4">
          <animate attributeName="opacity" values="0.3;0.7;0.3" dur="0.8s" repeatCount="indefinite"/>
        </circle>'''
    )
    # 3) 撼山 — crouch slam ground
    shan = mc_figure(
        body_color="#8b6914", skin_color="#d4a86a",
        torso_rot=30,
        left_arm_rot=-40, right_arm_rot=-40,
        left_arm_x=-2, right_arm_x=2,
        left_leg_rot=-30, right_leg_rot=30,
        extras='''
        <ellipse cx="0" cy="38" rx="18" ry="3" fill="none" stroke="#c9a96e" stroke-width="0.6" opacity="0.6">
          <animate attributeName="rx" values="8;22;8" dur="1.2s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.8;0.2;0.8" dur="1.2s" repeatCount="indefinite"/>
        </ellipse>
        <ellipse cx="0" cy="38" rx="12" ry="2" fill="none" stroke="#e8c97a" stroke-width="0.4" opacity="0.4">
          <animate attributeName="rx" values="5;16;5" dur="1.2s" repeatCount="indefinite" begin="0.2s"/>
        </ellipse>'''
    )
    # 4) 焚血 — wrist cut drip
    xue = mc_figure(
        body_color="#8b6914", skin_color="#d4a86a",
        left_arm_rot=-90, right_arm_rot=10,
        extras='''
        <circle cx="-8" cy="14" r="1" fill="#cc2222" opacity="0.8">
          <animate attributeName="cy" values="14;28;14" dur="1.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.9;0.2;0.9" dur="1.5s" repeatCount="indefinite"/>
        </circle>
        <circle cx="-9" cy="16" r="0.7" fill="#dd3333" opacity="0.6">
          <animate attributeName="cy" values="16;30;16" dur="1.8s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.8;0.1;0.8" dur="1.8s" repeatCount="indefinite"/>
        </circle>
        <circle cx="-7" cy="15" r="0.5" fill="#ee4444" opacity="0.7">
          <animate attributeName="cy" values="15;32;15" dur="2s" repeatCount="indefinite"/>
        </circle>
        <line x1="-10" y1="13" x2="-6" y2="15" stroke="#cc2222" stroke-width="0.5"/>'''
    )
    # 5) 散功 — arms spread, gold rays
    san = mc_figure(
        body_color="#8b6914", skin_color="#d4a86a",
        left_arm_rot=-100, right_arm_rot=100,
        left_arm_x=-1, right_arm_x=1,
        extras='''
        <line x1="0" y1="14" x2="-22" y2="5" stroke="#ffd700" stroke-width="0.6" opacity="0.6">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="0.7s" repeatCount="indefinite"/>
        </line>
        <line x1="0" y1="14" x2="22" y2="5" stroke="#ffd700" stroke-width="0.6" opacity="0.6">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="0.7s" repeatCount="indefinite" begin="0.1s"/>
        </line>
        <line x1="0" y1="14" x2="-18" y2="28" stroke="#ffd700" stroke-width="0.4" opacity="0.4">
          <animate attributeName="opacity" values="0.2;0.8;0.2" dur="0.9s" repeatCount="indefinite"/>
        </line>
        <line x1="0" y1="14" x2="18" y2="28" stroke="#ffd700" stroke-width="0.4" opacity="0.4">
          <animate attributeName="opacity" values="0.2;0.8;0.2" dur="0.9s" repeatCount="indefinite" begin="0.15s"/>
        </line>
        <line x1="0" y1="14" x2="0" y2="-4" stroke="#ffd700" stroke-width="0.6" opacity="0.5">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="0.6s" repeatCount="indefinite"/>
        </line>
        <circle cx="0" cy="14" r="8" fill="none" stroke="#ffd700" stroke-width="0.4" opacity="0.3">
          <animate attributeName="r" values="6;12;6" dur="1.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.1;0.4;0.1" dur="1.5s" repeatCount="indefinite"/>
        </circle>'''
    )
    return [beng, full, shan, xue, san]


# ─── Dugu (毒蛊) Poses ─────────────────────────────────────────────────────

def dugu_poses() -> list[str]:
    dark_green = "#1a4a2e"
    # 1) 蚀针 — throw needle
    zhen = mc_figure(
        body_color=dark_green, skin_color="#a8c4a0",
        right_arm_rot=-70, left_arm_rot=15,
        right_leg_rot=10, left_leg_rot=-10,
        extras=f'''
        <line x1="10" y1="12" x2="22" y2="8" stroke="#88cc88" stroke-width="0.6">
          <animate attributeName="x2" values="14;24;14" dur="0.5s" repeatCount="indefinite"/>
        </line>
        <polygon points="22,7 24,8 22,9" fill="#88cc88" opacity="0.8">
          <animate attributeName="opacity" values="0.5;1;0.5" dur="0.5s" repeatCount="indefinite"/>
        </polygon>'''
    )
    # 2) 自蕴 — sit cross-legged with bowl
    yun = mc_figure(
        body_color=dark_green, skin_color="#a8c4a0",
        torso_rot=0,
        left_arm_rot=-30, right_arm_rot=-30,
        left_leg_rot=60, right_leg_rot=-60,
        left_arm_x=2, right_arm_x=-2,
        extras=f'''
        <ellipse cx="0" cy="18" rx="4" ry="2" fill="#2a5a3e" stroke="#88cc88" stroke-width="0.4"/>
        <path d="M-3,17 Q0,15 3,17" fill="none" stroke="#556b2f" stroke-width="0.5">
          <animate attributeName="d" values="M-3,17 Q0,15 3,17;M-3,16 Q0,14 3,16;M-3,17 Q0,15 3,17" dur="2s" repeatCount="indefinite"/>
        </path>
        <circle cx="-1" cy="16" r="0.5" fill="#4a7a4e" opacity="0.6">
          <animate attributeName="cy" values="16;12;16" dur="3s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.6;0;0.6" dur="3s" repeatCount="indefinite"/>
        </circle>
        <circle cx="1" cy="15" r="0.4" fill="#4a7a4e" opacity="0.5">
          <animate attributeName="cy" values="15;10;15" dur="2.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.5;0;0.5" dur="2.5s" repeatCount="indefinite"/>
        </circle>'''
    )
    # 3) 侵染 — pulsing rings
    ran = mc_figure(
        body_color=dark_green, skin_color="#a8c4a0",
        left_arm_rot=-20, right_arm_rot=-20,
        extras=f'''
        <circle cx="0" cy="14" r="6" fill="none" stroke="#2d6b3e" stroke-width="0.5" opacity="0.5">
          <animate attributeName="r" values="4;10;4" dur="1.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.6;0.1;0.6" dur="1.5s" repeatCount="indefinite"/>
        </circle>
        <circle cx="0" cy="14" r="3" fill="none" stroke="#3d8b4e" stroke-width="0.4" opacity="0.4">
          <animate attributeName="r" values="3;8;3" dur="1.5s" repeatCount="indefinite" begin="0.3s"/>
          <animate attributeName="opacity" values="0.5;0.1;0.5" dur="1.5s" repeatCount="indefinite" begin="0.3s"/>
        </circle>
        <circle cx="0" cy="14" r="2" fill="none" stroke="#4dab5e" stroke-width="0.3" opacity="0.3">
          <animate attributeName="r" values="2;6;2" dur="1.5s" repeatCount="indefinite" begin="0.6s"/>
        </circle>'''
    )
    # 4) 神识遮蔽 — arms crossed, shadow
    zhe = mc_figure(
        body_color=dark_green, skin_color="#a8c4a0",
        left_arm_rot=-45, right_arm_rot=45,
        left_arm_x=4, right_arm_x=-4,
        extras=f'''
        <rect x="-8" y="2" width="16" height="24" rx="4" fill="#0a0a0a" opacity="0.3">
          <animate attributeName="opacity" values="0.2;0.5;0.2" dur="2s" repeatCount="indefinite"/>
        </rect>
        <ellipse cx="0" cy="14" rx="10" ry="14" fill="none" stroke="#1a3a2e" stroke-width="0.5" opacity="0.3" stroke-dasharray="2,2">
          <animate attributeName="opacity" values="0.2;0.5;0.2" dur="3s" repeatCount="indefinite"/>
        </ellipse>'''
    )
    # 5) 倒蚀 — point finger, green lightning
    dao = mc_figure(
        body_color=dark_green, skin_color="#a8c4a0",
        right_arm_rot=-85, left_arm_rot=10,
        right_arm_x=1,
        extras=f'''
        <line x1="10" y1="12" x2="24" y2="8" stroke="#44dd66" stroke-width="0.4" opacity="0.8"/>
        <polyline points="14,11 17,14 19,9 22,12 24,8" fill="none" stroke="#33ff55" stroke-width="0.5" opacity="0.7">
          <animate attributeName="opacity" values="0.4;1;0.4" dur="0.3s" repeatCount="indefinite"/>
        </polyline>
        <polyline points="15,13 18,10 20,14 23,10 25,7" fill="none" stroke="#22cc44" stroke-width="0.3" opacity="0.5">
          <animate attributeName="opacity" values="0.3;0.9;0.3" dur="0.4s" repeatCount="indefinite" begin="0.1s"/>
        </polyline>
        <circle cx="24" cy="8" r="2" fill="#44dd66" opacity="0.3">
          <animate attributeName="r" values="1;3;1" dur="0.5s" repeatCount="indefinite"/>
        </circle>'''
    )
    return [zhen, yun, ran, zhe, dao]


# ─── Tuike (替尸) Poses ─────────────────────────────────────────────────────

def tuike_poses() -> list[str]:
    brown = "#8b6914"
    # 1) 着壳 — layer appearing
    don = mc_figure(
        body_color="#6b5a3a", skin_color="#c8a882",
        left_arm_rot=-20, right_arm_rot=20,
        extras=f'''
        <rect x="-6" y="6" width="12" height="16" rx="2" fill="none" stroke="{brown}" stroke-width="0.6" opacity="0.5" stroke-dasharray="1.5,1">
          <animate attributeName="opacity" values="0.2;0.7;0.2" dur="2s" repeatCount="indefinite"/>
        </rect>
        <rect x="-7" y="5" width="14" height="18" rx="3" fill="none" stroke="{brown}" stroke-width="0.3" opacity="0.3" stroke-dasharray="2,1.5">
          <animate attributeName="opacity" values="0.1;0.5;0.1" dur="2.5s" repeatCount="indefinite" begin="0.3s"/>
        </rect>
        <circle cx="-5" cy="12" r="0.6" fill="{brown}" opacity="0.4">
          <animate attributeName="cy" values="18;8;18" dur="3s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0;0.6;0" dur="3s" repeatCount="indefinite"/>
        </circle>
        <circle cx="5" cy="10" r="0.5" fill="{brown}" opacity="0.3">
          <animate attributeName="cy" values="20;6;20" dur="2.8s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0;0.5;0" dur="2.8s" repeatCount="indefinite"/>
        </circle>'''
    )
    # 2) 蜕一层 — fragments bursting off
    shed = mc_figure(
        body_color="#6b5a3a", skin_color="#c8a882",
        left_arm_rot=-40, right_arm_rot=40,
        left_leg_rot=-10, right_leg_rot=10,
        extras=f'''
        <rect x="-12" y="8" width="3" height="2" rx="0.3" fill="{brown}" opacity="0.7" transform="rotate(-20,-12,8)">
          <animate attributeName="x" values="-6;-18;-6" dur="1s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.8;0.1;0.8" dur="1s" repeatCount="indefinite"/>
        </rect>
        <rect x="10" y="10" width="2.5" height="1.8" rx="0.3" fill="{brown}" opacity="0.6" transform="rotate(25,10,10)">
          <animate attributeName="x" values="6;16;6" dur="0.8s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.7;0.1;0.7" dur="0.8s" repeatCount="indefinite"/>
        </rect>
        <rect x="-10" y="18" width="2" height="3" rx="0.2" fill="{brown}" opacity="0.5" transform="rotate(-30,-10,18)">
          <animate attributeName="x" values="-5;-16;-5" dur="1.2s" repeatCount="indefinite"/>
          <animate attributeName="y" values="16;22;16" dur="1.2s" repeatCount="indefinite"/>
        </rect>
        <rect x="8" y="16" width="2.5" height="2" rx="0.2" fill="{brown}" opacity="0.6" transform="rotate(15,8,16)">
          <animate attributeName="x" values="5;14;5" dur="0.9s" repeatCount="indefinite"/>
          <animate attributeName="y" values="14;20;14" dur="0.9s" repeatCount="indefinite"/>
        </rect>
        <rect x="-8" y="5" width="1.5" height="1.5" rx="0.2" fill="#a07a34" opacity="0.5">
          <animate attributeName="x" values="-5;-14;-5" dur="1.1s" repeatCount="indefinite"/>
          <animate attributeName="y" values="6;2;6" dur="1.1s" repeatCount="indefinite"/>
        </rect>'''
    )
    # 3) 转移污染 — push dark flow to shell
    transfer = mc_figure(
        body_color="#6b5a3a", skin_color="#c8a882",
        right_arm_rot=-60, left_arm_rot=15,
        extras=f'''
        <circle cx="2" cy="14" r="1.5" fill="#3a2a1a" opacity="0.6">
          <animate attributeName="cx" values="2;12;2" dur="1.2s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.7;0.2;0.7" dur="1.2s" repeatCount="indefinite"/>
        </circle>
        <circle cx="0" cy="16" r="1" fill="#4a3a2a" opacity="0.5">
          <animate attributeName="cx" values="0;14;0" dur="1.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.6;0.1;0.6" dur="1.5s" repeatCount="indefinite"/>
        </circle>
        <path d="M3,14 Q8,12 13,14" fill="none" stroke="#5a4a2a" stroke-width="0.5" opacity="0.5">
          <animate attributeName="opacity" values="0.3;0.8;0.3" dur="1s" repeatCount="indefinite"/>
        </path>
        <rect x="14" y="6" width="6" height="14" rx="1.5" fill="none" stroke="{brown}" stroke-width="0.5" opacity="0.4" stroke-dasharray="1,1">
          <animate attributeName="opacity" values="0.3;0.7;0.3" dur="2s" repeatCount="indefinite"/>
        </rect>'''
    )
    return [don, shed, transfer]


# ─── Zhenfa (阵法) Poses ────────────────────────────────────────────────────

def zhenfa_poses() -> list[str]:
    blue_purple = "#4a3a7a"
    # 1) 护龛 — hex pattern on ground
    hu = mc_figure(
        body_color=blue_purple, skin_color="#b8a8d0",
        left_arm_rot=-30, right_arm_rot=-30,
        extras=f'''
        <polygon points="0,32 6,35 6,41 0,44 -6,41 -6,35" fill="none" stroke="#7a6aaa" stroke-width="0.5" opacity="0.6">
          <animate attributeName="opacity" values="0.4;0.9;0.4" dur="2s" repeatCount="indefinite"/>
        </polygon>
        <polygon points="0,34 4,36 4,40 0,42 -4,40 -4,36" fill="none" stroke="#9a8aca" stroke-width="0.3" opacity="0.4">
          <animate attributeName="opacity" values="0.3;0.7;0.3" dur="2s" repeatCount="indefinite" begin="0.5s"/>
        </polygon>
        <circle cx="0" cy="38" r="1" fill="#7a6aaa" opacity="0.5">
          <animate attributeName="opacity" values="0.3;0.8;0.3" dur="1.5s" repeatCount="indefinite"/>
        </circle>'''
    )
    # 2) 聚灵 — swirl converging
    ju = mc_figure(
        body_color=blue_purple, skin_color="#b8a8d0",
        left_arm_rot=-60, right_arm_rot=-60,
        left_arm_x=1, right_arm_x=-1,
        extras=f'''
        <circle cx="0" cy="10" r="8" fill="none" stroke="#8a7abb" stroke-width="0.3" opacity="0.4" stroke-dasharray="2,2">
          <animateTransform attributeName="transform" type="rotate" values="0 0 10;360 0 10" dur="4s" repeatCount="indefinite"/>
        </circle>
        <circle cx="0" cy="10" r="5" fill="none" stroke="#9a8acc" stroke-width="0.3" opacity="0.5" stroke-dasharray="1.5,1.5">
          <animateTransform attributeName="transform" type="rotate" values="360 0 10;0 0 10" dur="3s" repeatCount="indefinite"/>
        </circle>
        <circle cx="6" cy="6" r="0.8" fill="#aaaaff" opacity="0.5">
          <animate attributeName="cx" values="6;1;6" dur="2s" repeatCount="indefinite"/>
          <animate attributeName="cy" values="6;10;6" dur="2s" repeatCount="indefinite"/>
        </circle>
        <circle cx="-5" cy="14" r="0.6" fill="#aaaaff" opacity="0.4">
          <animate attributeName="cx" values="-5;0;-5" dur="2.5s" repeatCount="indefinite"/>
          <animate attributeName="cy" values="14;10;14" dur="2.5s" repeatCount="indefinite"/>
        </circle>
        <circle cx="3" cy="16" r="0.5" fill="#bbbbff" opacity="0.3">
          <animate attributeName="cx" values="3;0;3" dur="1.8s" repeatCount="indefinite"/>
          <animate attributeName="cy" values="16;10;16" dur="1.8s" repeatCount="indefinite"/>
        </circle>'''
    )
    # 3) 欺天 — distortion waves
    qi = mc_figure(
        body_color=blue_purple, skin_color="#b8a8d0",
        left_arm_rot=-110, right_arm_rot=-110,
        left_arm_x=2, right_arm_x=-2,
        head_rot=-5,
        extras=f'''
        <ellipse cx="0" cy="4" rx="10" ry="4" fill="none" stroke="#6a5a9a" stroke-width="0.4" opacity="0.3">
          <animate attributeName="ry" values="3;7;3" dur="1.5s" repeatCount="indefinite"/>
          <animate attributeName="opacity" values="0.2;0.5;0.2" dur="1.5s" repeatCount="indefinite"/>
        </ellipse>
        <ellipse cx="0" cy="2" rx="14" ry="3" fill="none" stroke="#5a4a8a" stroke-width="0.3" opacity="0.2">
          <animate attributeName="ry" values="2;5;2" dur="2s" repeatCount="indefinite" begin="0.3s"/>
          <animate attributeName="cy" values="2;-2;2" dur="2s" repeatCount="indefinite" begin="0.3s"/>
        </ellipse>
        <line x1="-8" y1="0" x2="8" y2="0" stroke="#7a6aaa" stroke-width="0.3" opacity="0.3" stroke-dasharray="1,2">
          <animate attributeName="y1" values="0;-3;0" dur="1.2s" repeatCount="indefinite"/>
          <animate attributeName="y2" values="0;-3;0" dur="1.2s" repeatCount="indefinite"/>
        </line>'''
    )
    # 4) 幻阵 — translucent overlay
    huan = mc_figure(
        body_color=blue_purple, skin_color="#b8a8d0",
        right_arm_rot=-50, left_arm_rot=10,
        extras=f'''
        <rect x="-12" y="2" width="24" height="28" rx="3" fill="none" stroke="#7a6aaa" stroke-width="0.3" opacity="0.2" stroke-dasharray="2,2">
          <animate attributeName="opacity" values="0.1;0.4;0.1" dur="3s" repeatCount="indefinite"/>
        </rect>
        <rect x="-10" y="4" width="20" height="24" rx="2" fill="#4a3a7a" opacity="0.08">
          <animate attributeName="opacity" values="0.05;0.15;0.05" dur="2.5s" repeatCount="indefinite"/>
        </rect>
        <circle cx="-6" cy="8" r="0.5" fill="#9a8aca" opacity="0.3">
          <animate attributeName="opacity" values="0;0.6;0" dur="2s" repeatCount="indefinite"/>
        </circle>
        <circle cx="5" cy="20" r="0.4" fill="#9a8aca" opacity="0.2">
          <animate attributeName="opacity" values="0;0.5;0" dur="2.3s" repeatCount="indefinite" begin="0.5s"/>
        </circle>
        <circle cx="-3" cy="24" r="0.6" fill="#8a7abb" opacity="0.25">
          <animate attributeName="opacity" values="0;0.5;0" dur="1.8s" repeatCount="indefinite" begin="1s"/>
        </circle>'''
    )
    return [hu, ju, qi, huan]


# ─── Particle Systems ───────────────────────────────────────────────────────

def particle_system(particle_id: str, color: str, behavior: str, count: int = 12) -> str:
    """Generate CSS/SVG animated particle container."""
    particles = []
    import random
    rng = random.Random(hash(particle_id))

    for i in range(count):
        x = rng.randint(5, 95)
        size = rng.uniform(2, 6)
        duration = rng.uniform(1.5, 4.0)
        delay = rng.uniform(0, 3.0)

        if behavior == "mist_up":
            y_start = rng.randint(60, 95)
            y_end = rng.randint(5, 40)
            particles.append(
                f'<div class="particle" style="left:{x}%;bottom:{100-y_start}%;width:{size}px;height:{size}px;'
                f'background:radial-gradient(circle,{color},{color}00);border-radius:50%;'
                f'animation:mist_up {duration:.1f}s {delay:.1f}s infinite ease-out;"></div>'
            )
        elif behavior == "drip_down":
            particles.append(
                f'<div class="particle" style="left:{x}%;top:10%;width:{max(2,size*0.6):.0f}px;height:{max(3,size*1.2):.0f}px;'
                f'background:{color};border-radius:0 0 50% 50%;'
                f'animation:drip_down {duration:.1f}s {delay:.1f}s infinite ease-in;"></div>'
            )
        elif behavior == "burst_out":
            angle = rng.uniform(0, 360)
            dist = rng.uniform(20, 48)
            particles.append(
                f'<div class="particle" style="left:50%;top:50%;width:{size}px;height:{size}px;'
                f'background:{color};border-radius:30%;'
                f'animation:burst_out {duration:.1f}s {delay:.1f}s infinite ease-out;'
                f'--angle:{angle}deg;--dist:{dist}%;"></div>'
            )
        elif behavior == "dust_settle":
            particles.append(
                f'<div class="particle" style="left:{x}%;top:20%;width:{size*1.3:.0f}px;height:{size*0.8:.0f}px;'
                f'background:{color};border-radius:40%;opacity:0.5;'
                f'animation:dust_settle {duration:.1f}s {delay:.1f}s infinite ease-in-out;"></div>'
            )
        elif behavior == "expand_ring":
            particles.append(
                f'<div class="particle" style="left:{48+rng.randint(-5,5)}%;top:{48+rng.randint(-5,5)}%;'
                f'width:{size*2:.0f}px;height:{size*2:.0f}px;'
                f'border:1px solid {color};border-radius:50%;background:transparent;'
                f'animation:expand_ring {duration:.1f}s {delay:.1f}s infinite ease-out;"></div>'
            )
        elif behavior == "spiral":
            particles.append(
                f'<div class="particle" style="left:50%;top:50%;width:{size}px;height:{size}px;'
                f'background:{color};border-radius:50%;'
                f'animation:spiral {duration:.1f}s {delay:.1f}s infinite linear;'
                f'--orbit-r:{15+rng.randint(0,25)}%;--start-angle:{rng.randint(0,360)}deg;"></div>'
            )

    return f'<div class="particle-box">{"".join(particles)}</div>'


# ─── Sound Mixer ────────────────────────────────────────────────────────────

def sound_mixer_html(name: str, layers: list[SoundLayer], accent: str) -> str:
    bars = []
    max_delay = max((l.delay_ticks for l in layers), default=0)
    for layer in layers:
        width_pct = int(layer.volume * 100)
        # Pitch maps to color intensity
        intensity = min(1.0, layer.pitch / 2.0)
        r, g, b = _hex_to_rgb(accent)
        lr = int(r * intensity + 255 * (1 - intensity) * 0.3)
        lg = int(g * intensity + 255 * (1 - intensity) * 0.3)
        lb = int(b * intensity + 255 * (1 - intensity) * 0.3)
        bar_color = f"rgb({min(255,lr)},{min(255,lg)},{min(255,lb)})"

        delay_gap = ""
        if layer.delay_ticks > 0:
            gap_w = int(layer.delay_ticks * 8)
            delay_gap = f'<div style="width:{gap_w}px;height:18px;border-left:1px dashed #555;margin:2px 0;display:flex;align-items:center;"><span style="font-size:9px;color:#777;padding-left:3px;">+{layer.delay_ticks}t</span></div>'

        bars.append(f'''{delay_gap}<div class="sound-bar" style="display:flex;align-items:center;margin:3px 0;">
          <div style="width:{width_pct}%;min-width:30px;max-width:100%;height:18px;background:linear-gradient(90deg,{bar_color},{bar_color}88);border-radius:2px;position:relative;overflow:hidden;">
            <div style="position:absolute;top:0;left:0;height:100%;width:30%;background:rgba(255,255,255,0.1);animation:shimmer 2s infinite;"></div>
          </div>
          <span style="font-size:10px;color:#aaa;margin-left:6px;white-space:nowrap;">{layer.sound_id} <span style="color:#666;">p={layer.pitch} v={layer.volume}</span></span>
        </div>''')

    return f'<div class="sound-mixer"><div class="mixer-label">{name}</div>{"".join(bars)}</div>'


def _hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip('#')
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


# ─── HUD Mockups ────────────────────────────────────────────────────────────

def hud_game_screen(hud_elements: list[str], accent: str) -> str:
    elements_html = "\n".join(hud_elements)
    return f'''<div class="game-screen">
  <div class="game-screen-label">游戏画面 HUD</div>
  {elements_html}
</div>'''


def hud_charge_ring(label: str, pct: int, color: str) -> str:
    return f'''<div class="hud-element" style="position:absolute;right:20px;top:20px;">
  <div class="charge-ring" style="--pct:{pct};--color:{color};">
    <svg viewBox="0 0 40 40" width="50" height="50">
      <circle cx="20" cy="20" r="17" fill="none" stroke="#333" stroke-width="3"/>
      <circle cx="20" cy="20" r="17" fill="none" stroke="{color}" stroke-width="3"
        stroke-dasharray="{pct * 1.07} 107" stroke-dashoffset="-26.75"
        stroke-linecap="round">
        <animate attributeName="stroke-dasharray" values="0 107;{pct * 1.07} 107;0 107" dur="3s" repeatCount="indefinite"/>
      </circle>
    </svg>
    <span class="ring-label">{label}</span>
  </div>
</div>'''


def hud_health_qi_bars(hp_pct: int, qi_pct: int, accent: str) -> str:
    return f'''<div class="hud-element" style="position:absolute;left:15px;bottom:20px;width:180px;">
  <div class="bar-container">
    <div class="bar-label">生命</div>
    <div class="bar-track"><div class="bar-fill" style="width:{hp_pct}%;background:linear-gradient(90deg,#cc3333,#ff5555);"></div></div>
  </div>
  <div class="bar-container" style="margin-top:4px;">
    <div class="bar-label">真元</div>
    <div class="bar-track"><div class="bar-fill" style="width:{qi_pct}%;background:linear-gradient(90deg,#3366cc,#55aaff);"></div></div>
  </div>
</div>'''


def hud_taint_indicator(pct: int, color: str, label: str) -> str:
    return f'''<div class="hud-element" style="position:absolute;right:80px;top:20px;">
  <div class="taint-bar-label">{label}</div>
  <div class="taint-track">
    <div class="taint-fill" style="height:{pct}%;background:linear-gradient(0deg,{color}44,{color});"></div>
    <span class="taint-pct">{pct}%</span>
  </div>
</div>'''


def hud_timer_ring(label: str, seconds: int, color: str) -> str:
    return f'''<div class="hud-element" style="position:absolute;right:20px;bottom:20px;">
  <div class="timer-ring">
    <svg viewBox="0 0 44 44" width="48" height="48">
      <circle cx="22" cy="22" r="19" fill="none" stroke="#333" stroke-width="2.5"/>
      <circle cx="22" cy="22" r="19" fill="none" stroke="{color}" stroke-width="2.5"
        stroke-dasharray="119.4" stroke-dashoffset="0" stroke-linecap="round">
        <animate attributeName="stroke-dashoffset" from="0" to="119.4" dur="{seconds}s" repeatCount="indefinite"/>
      </circle>
      <text x="22" y="25" text-anchor="middle" fill="#ccc" font-size="8" font-family="monospace">{seconds}s</text>
    </svg>
    <span class="timer-label">{label}</span>
  </div>
</div>'''


def hud_stack_display(layers: list[tuple[str, int]], accent: str) -> str:
    icons = []
    for i, (name, durability) in enumerate(layers):
        offset = i * 18
        icons.append(
            f'<div class="stack-icon" style="left:{offset}px;">'
            f'<div class="stack-bar" style="height:{durability}%;background:{accent};"></div>'
            f'<span class="stack-name">{name}</span></div>'
        )
    return f'''<div class="hud-element" style="position:absolute;left:15px;top:20px;">
  <div class="stack-display">{"".join(icons)}</div>
</div>'''


def hud_array_layout(positions: list[tuple[int, int]], color: str) -> str:
    dots = []
    for x, y in positions:
        dots.append(f'<circle cx="{x}" cy="{y}" r="2" fill="{color}" opacity="0.7"><animate attributeName="opacity" values="0.5;1;0.5" dur="2s" repeatCount="indefinite"/></circle>')
    lines = []
    for i in range(len(positions)):
        for j in range(i + 1, len(positions)):
            x1, y1 = positions[i]
            x2, y2 = positions[j]
            lines.append(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="0.5" opacity="0.3"/>')
    return f'''<div class="hud-element" style="position:absolute;left:220px;bottom:15px;">
  <div class="array-mini-label">阵图</div>
  <svg viewBox="0 0 60 60" width="65" height="65" style="background:#111;border:1px solid #333;border-radius:4px;">
    {"".join(lines)}
    {"".join(dots)}
  </svg>
</div>'''


def hud_body_silhouette(wounds: list[tuple[int, int]], accent: str) -> str:
    wound_dots = "".join(
        f'<circle cx="{x}" cy="{y}" r="1.5" fill="#cc3333" opacity="0.8">'
        f'<animate attributeName="opacity" values="0.5;1;0.5" dur="1.5s" repeatCount="indefinite"/></circle>'
        for x, y in wounds
    )
    return f'''<div class="hud-element" style="position:absolute;left:210px;top:15px;">
  <svg viewBox="0 0 30 55" width="35" height="60" style="filter:drop-shadow(0 0 2px {accent}33);">
    <!-- body silhouette -->
    <ellipse cx="15" cy="7" rx="5" ry="5.5" fill="#222" stroke="#444" stroke-width="0.5"/>
    <rect x="10" y="12" width="10" height="14" rx="2" fill="#222" stroke="#444" stroke-width="0.5"/>
    <rect x="5" y="13" width="5" height="12" rx="1" fill="#222" stroke="#444" stroke-width="0.5"/>
    <rect x="20" y="13" width="5" height="12" rx="1" fill="#222" stroke="#444" stroke-width="0.5"/>
    <rect x="11" y="26" width="4" height="14" rx="1" fill="#222" stroke="#444" stroke-width="0.5"/>
    <rect x="16" y="26" width="4" height="14" rx="1" fill="#222" stroke="#444" stroke-width="0.5"/>
    {wound_dots}
  </svg>
</div>'''


# ─── HTML Template ──────────────────────────────────────────────────────────

def generate_html(style: Style) -> str:
    skills_html = []
    for skill in style.skills:
        # Sound mixer
        sound_html = sound_mixer_html(
            f"{skill.name_cn} 音效",
            skill.sound_layers,
            style.accent
        )

        skills_html.append(f'''
<div class="skill-card">
  <h2 class="skill-title">{skill.name_cn} <span class="skill-en">{skill.name_en}</span></h2>
  <div class="skill-row">
    <div class="col pose-col">
      <div class="col-label">角色姿态</div>
      <div class="pose-container">{skill.pose_svg}</div>
    </div>
    <div class="col particle-col">
      <div class="col-label">粒子特效</div>
      {skill.particle_html}
    </div>
    <div class="col sound-col">
      <div class="col-label">音效层叠</div>
      {sound_html}
    </div>
  </div>
  <div class="hud-row">
    {hud_game_screen([h.extra.get("html", "") for h in skill.hud], style.accent)}
  </div>
</div>''')

    return f'''<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>{style.name_cn}（{style.name_en}）AV 体验图</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
  background: #0a0a0a;
  color: #e0e0e0;
  font-family: "Cascadia Code", "Fira Code", "SF Mono", monospace;
  padding: 20px;
  min-width: 900px;
}}
h1 {{
  text-align: center;
  font-size: 22px;
  border-bottom: 2px solid {style.accent};
  padding-bottom: 10px;
  margin-bottom: 24px;
  color: {style.accent};
  text-shadow: 0 0 12px {style.accent}55;
}}
.skill-card {{
  border: 1px solid {style.accent}44;
  border-radius: 8px;
  margin-bottom: 28px;
  padding: 16px;
  background: #111111;
  box-shadow: 0 0 20px {style.accent}11;
}}
.skill-title {{
  font-size: 16px;
  color: {style.accent};
  margin-bottom: 14px;
  padding-left: 8px;
  border-left: 3px solid {style.accent};
}}
.skill-en {{ color: #777; font-size: 12px; font-weight: normal; }}
.skill-row {{
  display: grid;
  grid-template-columns: 200px 1fr 1fr;
  gap: 16px;
  margin-bottom: 14px;
}}
.col {{
  background: #0d0d0d;
  border-radius: 6px;
  padding: 12px;
  border: 1px solid #222;
  min-height: 180px;
}}
.col-label {{
  font-size: 10px;
  color: #666;
  text-transform: uppercase;
  letter-spacing: 1px;
  margin-bottom: 8px;
}}
.pose-container {{
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 160px;
}}
.pose-container svg {{
  filter: drop-shadow(0 0 6px {style.accent}33);
}}

/* Particle container */
.particle-box {{
  position: relative;
  width: 100%;
  height: 160px;
  overflow: hidden;
  border-radius: 4px;
  background: radial-gradient(ellipse at center, #0f0f0f, #080808);
}}
.particle {{
  position: absolute;
  pointer-events: none;
}}

/* Particle animations */
@keyframes mist_up {{
  0% {{ transform: translateY(0) scale(1); opacity: 0; }}
  15% {{ opacity: 0.7; }}
  85% {{ opacity: 0.3; }}
  100% {{ transform: translateY(-80px) scale(1.8); opacity: 0; }}
}}
@keyframes drip_down {{
  0% {{ transform: translateY(0); opacity: 0; }}
  10% {{ opacity: 0.8; }}
  90% {{ opacity: 0.4; }}
  100% {{ transform: translateY(130px); opacity: 0; }}
}}
@keyframes burst_out {{
  0% {{ transform: translate(0, 0) scale(1); opacity: 0.8; }}
  100% {{ transform: translate(calc(cos(var(--angle)) * var(--dist)), calc(sin(var(--angle)) * var(--dist))) scale(0.3); opacity: 0; }}
}}
@keyframes dust_settle {{
  0% {{ transform: translateY(0) translateX(0); opacity: 0; }}
  20% {{ opacity: 0.6; }}
  80% {{ opacity: 0.3; }}
  100% {{ transform: translateY(100px) translateX(10px); opacity: 0; }}
}}
@keyframes expand_ring {{
  0% {{ transform: scale(0.5); opacity: 0.8; }}
  100% {{ transform: scale(4); opacity: 0; }}
}}
@keyframes spiral {{
  0% {{ transform: rotate(var(--start-angle)) translateX(var(--orbit-r)) rotate(calc(-1 * var(--start-angle))); opacity: 0.6; }}
  50% {{ opacity: 0.9; }}
  100% {{ transform: rotate(calc(var(--start-angle) + 360deg)) translateX(var(--orbit-r)) rotate(calc(-1 * var(--start-angle) - 360deg)); opacity: 0.6; }}
}}
@keyframes shimmer {{
  0% {{ transform: translateX(-100%); }}
  100% {{ transform: translateX(400%); }}
}}

/* Sound mixer */
.sound-mixer {{ padding: 4px 0; }}
.mixer-label {{ font-size: 11px; color: #888; margin-bottom: 6px; }}
.sound-bar {{ font-size: 10px; }}

/* Game screen HUD */
.hud-row {{ margin-top: 8px; }}
.game-screen {{
  position: relative;
  width: 100%;
  height: 140px;
  background: linear-gradient(180deg, #0c0c0c 0%, #141414 50%, #0c0c0c 100%);
  border: 1px solid #333;
  border-radius: 6px;
  overflow: hidden;
}}
.game-screen-label {{
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  color: #1a1a1a;
  font-size: 28px;
  font-weight: bold;
  letter-spacing: 6px;
  pointer-events: none;
  z-index: 0;
}}
.hud-element {{ z-index: 1; }}

/* Charge ring */
.charge-ring {{ text-align: center; }}
.ring-label {{ display: block; font-size: 9px; color: #aaa; margin-top: 2px; }}

/* Bars */
.bar-container {{ display: flex; align-items: center; gap: 6px; }}
.bar-label {{ font-size: 9px; color: #888; width: 28px; text-align: right; }}
.bar-track {{ flex: 1; height: 6px; background: #222; border-radius: 3px; overflow: hidden; }}
.bar-fill {{ height: 100%; border-radius: 3px; transition: width 0.5s; }}

/* Taint indicator */
.taint-bar-label {{ font-size: 9px; color: #888; text-align: center; margin-bottom: 3px; }}
.taint-track {{ width: 14px; height: 70px; background: #1a1a1a; border-radius: 3px; border: 1px solid #333; position: relative; overflow: hidden; }}
.taint-fill {{ position: absolute; bottom: 0; width: 100%; border-radius: 2px; }}
.taint-pct {{ position: absolute; bottom: -14px; left: -3px; font-size: 8px; color: #888; width: 20px; text-align: center; }}

/* Timer ring */
.timer-ring {{ text-align: center; }}
.timer-label {{ display: block; font-size: 9px; color: #aaa; margin-top: 2px; }}

/* Stack display */
.stack-display {{ position: relative; height: 55px; width: 120px; }}
.stack-icon {{
  position: absolute;
  bottom: 0;
  width: 16px;
  height: 45px;
  background: #1a1a1a;
  border: 1px solid #444;
  border-radius: 2px;
  overflow: hidden;
}}
.stack-bar {{ position: absolute; bottom: 0; width: 100%; border-radius: 1px; }}
.stack-name {{ position: absolute; bottom: -12px; left: -2px; font-size: 7px; color: #888; width: 20px; text-align: center; white-space: nowrap; }}

/* Array mini */
.array-mini-label {{ font-size: 9px; color: #888; margin-bottom: 3px; }}

</style>
</head>
<body>
<h1>{style.name_cn}（{style.name_en}）— 视听体验总览</h1>
{"".join(skills_html)}
</body>
</html>'''


# ─── Style Definitions ──────────────────────────────────────────────────────

def build_baomai() -> Style:
    accent = "#c9a96e"
    poses = baomai_poses()

    return Style(
        name_cn="体修 · 爆脉", name_en="Baomai", accent=accent,
        skills=[
            Skill(
                name_cn="崩拳", name_en="Beng Quan (Forward Punch)",
                pose_svg=poses[0],
                particle_html=particle_system("baomai_beng", "#c9a96e", "expand_ring", 8),
                sound_layers=[
                    SoundLayer("entity.generic.explode", 0.5, 0.7),
                    SoundLayer("block.stone.break", 0.6, 0.5, delay_ticks=2),
                ],
                hud=[HudComponent("BloodBurnRatioHud", "焚血比例", "charge_ring", extra={
                    "html": hud_charge_ring("蓄力", 65, accent)
                    + hud_health_qi_bars(80, 60, accent)
                    + hud_body_silhouette([(12, 18), (18, 16)], accent)
                })],
            ),
            Skill(
                name_cn="全力一击", name_en="Full Power Strike",
                pose_svg=poses[1],
                particle_html=particle_system("baomai_full", "#ffd700", "expand_ring", 10),
                sound_layers=[
                    SoundLayer("entity.lightning_bolt.thunder", 1.3, 0.8),
                ],
                hud=[HudComponent("BodyTranscendenceTimerHud", "蓄力球", "charge_ring", extra={
                    "html": hud_charge_ring("蓄力", 90, "#ffd700")
                    + hud_health_qi_bars(70, 30, accent)
                    + hud_timer_ring("虚脱", 30, "#888888")
                })],
            ),
            Skill(
                name_cn="撼山", name_en="Mountain Shake (Ground Slam)",
                pose_svg=poses[2],
                particle_html=particle_system("baomai_shan", "#a08050", "dust_settle", 15),
                sound_layers=[
                    SoundLayer("entity.generic.explode", 0.5, 0.7),
                    SoundLayer("block.stone.break", 0.6, 0.5, delay_ticks=2),
                ],
                hud=[HudComponent("MeridianRippleScarHud", "震波 AOE", "charge_ring", extra={
                    "html": hud_charge_ring("震波", 45, accent)
                    + hud_health_qi_bars(85, 50, accent)
                    + hud_body_silhouette([(13, 28), (17, 28), (15, 35)], accent)
                })],
            ),
            Skill(
                name_cn="焚血", name_en="Blood Burn",
                pose_svg=poses[3],
                particle_html=particle_system("baomai_xue", "#cc2222", "drip_down", 12),
                sound_layers=[
                    SoundLayer("entity.player.hurt", 0.8, 0.6),
                    SoundLayer("block.fire.ambient", 1.5, 0.4, delay_ticks=1),
                ],
                hud=[HudComponent("BloodBurnRatioHud", "焚血 HP 比例", "bar", extra={
                    "html": hud_health_qi_bars(35, 85, accent)
                    + hud_taint_indicator(40, "#cc2222", "焚血")
                    + hud_timer_ring("焚血", 20, "#cc3333")
                    + hud_body_silhouette([(8, 18), (22, 18), (15, 22), (12, 30), (18, 30)], accent)
                })],
            ),
            Skill(
                name_cn="散功", name_en="Disperse Qi (Transcendence)",
                pose_svg=poses[4],
                particle_html=particle_system("baomai_san", "#ffd700", "mist_up", 14),
                sound_layers=[
                    SoundLayer("entity.lightning_bolt.thunder", 1.3, 0.8),
                ],
                hud=[HudComponent("BodyTranscendenceTimerHud", "凡躯重铸倒计", "timer", extra={
                    "html": hud_timer_ring("重铸", 5, "#ffd700")
                    + hud_health_qi_bars(50, 15, accent)
                    + hud_taint_indicator(50, "#ffd700", "qi_max")
                    + hud_body_silhouette([(8, 14), (22, 14), (10, 20), (20, 20), (12, 28), (18, 28), (15, 7)], accent)
                })],
            ),
        ]
    )


def build_dugu() -> Style:
    accent = "#1a4a2e"
    poses = dugu_poses()
    accent_bright = "#2d8b4e"

    return Style(
        name_cn="毒蛊", name_en="Dugu", accent=accent,
        skills=[
            Skill(
                name_cn="蚀针", name_en="Eclipse Needle",
                pose_svg=poses[0],
                particle_html=particle_system("dugu_needle", "#2d6b3e", "mist_up", 10),
                sound_layers=[
                    SoundLayer("entity.spider.hurt", 1.5, 0.4),
                    SoundLayer("block.fire.extinguish", 1.2, 0.3, delay_ticks=1),
                ],
                hud=[HudComponent("DuguTaintWarningHud", "蚀针命中", "bar", extra={
                    "html": hud_health_qi_bars(85, 70, accent_bright)
                    + hud_taint_indicator(15, accent_bright, "蛊毒")
                    + hud_charge_ring("暴露", 25, "#cc3333")
                })],
            ),
            Skill(
                name_cn="自蕴", name_en="Self-Cure Cultivation",
                pose_svg=poses[1],
                particle_html=particle_system("dugu_cure", "#3d7a4e", "mist_up", 8),
                sound_layers=[
                    SoundLayer("entity.witch.drink", 0.8, 0.5),
                ],
                hud=[HudComponent("SelfCureProgressHud", "自蕴进度", "taint_indicator", extra={
                    "html": hud_taint_indicator(35, accent_bright, "阴诡色")
                    + hud_health_qi_bars(90, 80, accent_bright)
                    + hud_timer_ring("服食", 60, accent_bright)
                })],
            ),
            Skill(
                name_cn="侵染", name_en="Penetrate (Chain Infection)",
                pose_svg=poses[2],
                particle_html=particle_system("dugu_pen", "#2d6b3e", "expand_ring", 10),
                sound_layers=[
                    SoundLayer("entity.spider.hurt", 1.5, 0.4),
                    SoundLayer("block.fire.extinguish", 1.2, 0.3, delay_ticks=1),
                ],
                hud=[HudComponent("DuguTaintWarningHud", "联级倍率", "charge_ring", extra={
                    "html": hud_charge_ring("联级", 80, accent_bright)
                    + hud_health_qi_bars(75, 55, accent_bright)
                    + hud_taint_indicator(45, accent_bright, "蛊毒")
                })],
            ),
            Skill(
                name_cn="神识遮蔽", name_en="Consciousness Shroud",
                pose_svg=poses[3],
                particle_html=particle_system("dugu_shroud", "#1a3a2e", "mist_up", 6),
                sound_layers=[
                    SoundLayer("entity.witch.celebrate", 0.7, 0.6),
                    SoundLayer("ambient.cave", 1.2, 0.3, delay_ticks=5),
                ],
                hud=[HudComponent("RevealRiskHud", "暴露概率", "charge_ring", extra={
                    "html": hud_charge_ring("遮蔽", 70, "#556b2f")
                    + hud_health_qi_bars(90, 65, accent_bright)
                    + hud_taint_indicator(20, "#cc3333", "暴露")
                })],
            ),
            Skill(
                name_cn="倒蚀", name_en="Reverse Corrosion",
                pose_svg=poses[4],
                particle_html=particle_system("dugu_reverse", "#33ff55", "burst_out", 14),
                sound_layers=[
                    SoundLayer("entity.witch.celebrate", 0.7, 0.6),
                    SoundLayer("ambient.cave", 1.2, 0.3, delay_ticks=5),
                ],
                hud=[HudComponent("RevealRiskHud", "倒蚀引爆", "charge_ring", extra={
                    "html": hud_charge_ring("引爆", 95, "#44dd66")
                    + hud_health_qi_bars(60, 20, accent_bright)
                    + hud_taint_indicator(85, accent_bright, "阴诡色")
                    + hud_timer_ring("绝壁劫", 30, "#cc3333")
                })],
            ),
        ]
    )


def build_tuike() -> Style:
    accent = "#8b6914"
    poses = tuike_poses()

    return Style(
        name_cn="替尸 · 蜕壳", name_en="Tuike", accent=accent,
        skills=[
            Skill(
                name_cn="着壳", name_en="Don False Skin",
                pose_svg=poses[0],
                particle_html=particle_system("tuike_don", "#a08050", "dust_settle", 10),
                sound_layers=[
                    SoundLayer("item.armor.equip_leather", 0.7, 0.6),
                ],
                hud=[HudComponent("FalseSkinStackHud", "多层叠穿", "stack", extra={
                    "html": hud_stack_display([("轻", 90), ("中", 75), ("重", 60)], accent)
                    + hud_health_qi_bars(95, 85, accent)
                })],
            ),
            Skill(
                name_cn="蜕一层", name_en="Shed One Layer",
                pose_svg=poses[1],
                particle_html=particle_system("tuike_shed", "#8b7a44", "burst_out", 14),
                sound_layers=[
                    SoundLayer("block.wool.break", 1.2, 0.7),
                    SoundLayer("item.armor.equip_leather", 1.5, 0.4, delay_ticks=1),
                ],
                hud=[HudComponent("FalseSkinStackHud", "蜕壳瞬间", "stack", extra={
                    "html": hud_stack_display([("轻", 50), ("中", 0)], accent)
                    + hud_health_qi_bars(60, 40, accent)
                    + hud_timer_ring("裸壳", 5, "#cc3333")
                })],
            ),
            Skill(
                name_cn="转移污染", name_en="Transfer Contamination",
                pose_svg=poses[2],
                particle_html=particle_system("tuike_transfer", "#4a3a2a", "mist_up", 8),
                sound_layers=[
                    SoundLayer("block.beacon.activate", 1.0, 0.4),
                ],
                hud=[HudComponent("ContamLoadHud", "污染承载", "taint_indicator", extra={
                    "html": hud_taint_indicator(60, "#6b4a1a", "污染")
                    + hud_stack_display([("上古", 40)], "#ffd700")
                    + hud_health_qi_bars(70, 30, accent)
                })],
            ),
        ]
    )


def build_zhenfa() -> Style:
    accent = "#4a3a7a"
    poses = zhenfa_poses()
    accent_bright = "#7a6aaa"

    return Style(
        name_cn="地师 · 阵法", name_en="Zhenfa", accent=accent,
        skills=[
            Skill(
                name_cn="护龛阵", name_en="Ward Array",
                pose_svg=poses[0],
                particle_html=particle_system("zhenfa_ward", "#7a6aaa", "expand_ring", 10),
                sound_layers=[
                    SoundLayer("block.beacon.activate", 0.8, 0.5),
                    SoundLayer("block.anvil.land", 1.5, 0.3, delay_ticks=3),
                ],
                hud=[HudComponent("ArrayLayoutHud", "阵图布局", "array_layout", extra={
                    "html": hud_array_layout([(10, 30), (30, 10), (50, 30), (30, 50), (30, 30)], accent_bright)
                    + hud_health_qi_bars(85, 60, accent_bright)
                    + hud_timer_ring("朽坏", 720, accent_bright)
                })],
            ),
            Skill(
                name_cn="聚灵阵", name_en="Spirit Gathering Array",
                pose_svg=poses[1],
                particle_html=particle_system("zhenfa_ling", "#8888ff", "spiral", 14),
                sound_layers=[
                    SoundLayer("block.beacon.activate", 1.0, 0.5),
                    SoundLayer("block.amethyst_block.chime", 1.5, 0.4, delay_ticks=2),
                ],
                hud=[HudComponent("TianDaoZhuShiHud", "天道注视", "charge_ring", extra={
                    "html": hud_charge_ring("天道", 35, "#cc3333")
                    + hud_array_layout([(10, 10), (50, 10), (30, 30), (10, 50), (50, 50), (30, 10), (30, 50), (10, 30), (50, 30)], accent_bright)
                    + hud_health_qi_bars(70, 40, accent_bright)
                })],
            ),
            Skill(
                name_cn="欺天阵", name_en="Heaven Deceive Array",
                pose_svg=poses[2],
                particle_html=particle_system("zhenfa_deceive", "#6a5a9a", "mist_up", 12),
                sound_layers=[
                    SoundLayer("block.beacon.activate", 0.6, 0.6),
                    SoundLayer("entity.lightning_bolt.thunder", 1.0, 0.3, delay_ticks=8),
                ],
                hud=[HudComponent("DeceiveHudHud", "假劫期", "timer", extra={
                    "html": hud_timer_ring("欺天", 60, accent_bright)
                    + hud_charge_ring("识破", 12, "#cc3333")
                    + hud_health_qi_bars(40, 10, accent_bright)
                    + hud_taint_indicator(25, "#cc3333", "业力")
                })],
            ),
            Skill(
                name_cn="幻阵", name_en="Illusion Array",
                pose_svg=poses[3],
                particle_html=particle_system("zhenfa_illusion", "#9a8aca", "mist_up", 8),
                sound_layers=[
                    SoundLayer("block.glass.break", 1.2, 0.4),
                    SoundLayer("entity.enderman.teleport", 0.6, 0.3, delay_ticks=2),
                ],
                hud=[HudComponent("IllusionHud", "幻阵隐蔽", "charge_ring", extra={
                    "html": hud_charge_ring("隐蔽", 55, accent_bright)
                    + hud_array_layout([(15, 15), (45, 15), (30, 45)], accent_bright)
                    + hud_health_qi_bars(88, 72, accent_bright)
                })],
            ),
        ]
    )


# ─── Main ───────────────────────────────────────────────────────────────────

def main():
    styles = [
        ("baomai", build_baomai()),
        ("dugu",   build_dugu()),
        ("tuike",  build_tuike()),
        ("zhenfa", build_zhenfa()),
    ]

    for slug, style in styles:
        html = generate_html(style)
        out_path = SCRIPT_DIR / f"av-{slug}.html"
        out_path.write_text(html, encoding="utf-8")
        size_kb = out_path.stat().st_size / 1024
        print(f"  {out_path.name}: {size_kb:.1f} KB")

    print("Done. 4 HTML files generated.")


if __name__ == "__main__":
    main()
