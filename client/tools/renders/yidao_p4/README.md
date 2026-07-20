# P4 yidao 5 招终轮三视图人工验收证据

`plan-skill-anim-fidelity-v1` P0 精度标准要求：「无法机械判定的重心转移/全身协调，
列为逐招人工验收证据：批次 PR 附 `render_animation.py` 三视图 PNG + 对照 checklist」。
本目录是 P4 批次（PR-6）**终轮代码**产出的 10 份三视图 grid（每份含 FRONT / SIDE /
TOP 三视角 × 全部关键帧）。

重生成（决定性，与本目录字节一致）：

```bash
cd client
for n in yidao_meridian_repair yidao_contam_purge yidao_emergency_resuscitate \
         yidao_life_extension yidao_mass_meridian_repair; do
  for seg in loop release; do
    python3 tools/render_animation.py \
      "src/main/resources/assets/bong/player_animation/${n}_${seg}.json" \
      -o "tools/renders/yidao_p4/${n}_${seg}"
  done
done
```

## 逐招人工验收 checklist（对照 plan-yidao-v1 §5 ①-⑤）

| 招 | 双手职责 | 姿态母题兑现 | 重心/全身协调 | 远距离可辨性（与同批其余 4 招对比） |
|---|---|---|---|---|
| ① 接经术 loop 90t | **双手各持针交替落针**（右 15 针 / 左 15 针，每 3t 一针） | 30 穴位序：落点沿经脉三角 sweep 外推再回程（yaw 行程 14°），i=30 与 i=0 同相位闭环 | torso +22→+24 随外探渐沉、body.y -0.115→-0.127、legs bow 补偿（pitch -8/-7 + bend 16/15） | 唯一「双手高频交替下顿」节律；幅度 pitch 20°/bend 14°/roll 18° |
| ① 接经术 release 14t | 双手同拔（右主挑 -28→-96 / 左随起 -33→-82） | 定针一按（t2 双腕同沉）→ 拔针顶点 t8 → 拂袖归中立 t14 | torso +23→-2 直腰大位移（本批唯一俯→立迁移） | 唯一「直腰」收势 |
| ② 排异加速 loop 24t | 双手**对称同步**推送（非交替） | 对掌灸火：双臂 bend 16↔62 大幅推收 | 直立位（torso 仅 +6~+11），无俯身 | 唯一「直立对称推掌」 |
| ② 排异 release 12t | 双臂横向开扇 | 散烟：yaw 外张 | 直立收势 | 横向开扇轨迹 |
| ③ 急救 loop 20t | 双手**中线叠压**（非分开） | CPR 按压：body.y -0.28↔-0.40 深浅起伏 | torso +24~+30 深俯、直臂（bend 小） | 唯一「深俯 + 垂直起伏」 |
| ③ 急救 release 12t | 侧耳俯听 | head.yaw 偏转听息 | 由俯听直起 | 唯一「侧头」收势 |
| ④ 续命术 loop 26t | **右臂朝天接引 + 左手喂丹**（唯一不对称高举） | 接天引：rArm pitch -147~-158 | 直立微仰 | 唯一「单臂朝天」 |
| ④ 续命 release 14t | 单臂自天纵贯合封 | rArm -152→-165→0 单向下落 | 归中立 | 唯一「自天纵贯」轨迹 |
| ⑤ 群体接经 loop 32t | 双手**捧法器举过头顶** | 环阵横扫：torso.yaw ±16° 环视 | 双臂 pitch ≈ -148~-151（唯一举过头顶） | 唯一「双臂举顶 + 环视」 |
| ⑤ 群体 release 12t | 对称沉落抱器 | 双臂对称下落 | 归中立 | 对称沉落 |

机械项（已由 `AnimCastTicksAlignmentTest` + 生成脚本自断言锁定，不依赖目检）：
循环缝合按 returnTick 回绕锚点逐轴同值 / `leg.pitch` ≤ 40° / 关键帧密度 ≤ 4t
（本批最大 4t，接经术 3t）/ easing 显式且主打击轴非 linear / loop 基位 → release
承接帧逐轴对拍（5 招中 4 招零偏差，`mass_meridian_repair` 7 轴微差系 `torso.yaw`
环视摆动取中位锚点，详见 plan P4 打磨记录）。
