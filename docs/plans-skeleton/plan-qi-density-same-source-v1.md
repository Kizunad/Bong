# plan-qi-density-same-source-v1 — qi_density 层真同源烘焙（骨架）

一句话主题：把 raster `qi_density` 层从「20+ terrain profile 各自手搓」迁移为「从 `build_qi_field` 统一灵气场烘焙 + profile 局部调制」，让 worldgen-v4 P4 的同源派生断言（§8.1 #8）真正激活。

## 背景（2026-07-06 bot playtest 排查定案）

- P4（#537）引入统一场（`qi_field.build_qi_field`）用于 zones.json spirit_qi 派生与预算配平报表，并在 raster_check 加了「qi_density 均值 ≈ clamp01((spirit_qi+1)/2)」同源硬断言——但**统一场从未烘进 raster qi_density 层**，各 profile 仍各自手搓（`buffer.layers["qi_density"] = ...` 共 20+ 处）。
- raster_export 曾写死 `qi_density_source: "qi_field"` 假声明 → 全图 regen 后 345 处误报（fix-raster-qi-check-balanced 已改为如实声明 "profile"，断言按设计休眠，qi_density 值域放宽 [-1,1] 容纳负灵域）。
- 两套体系并存 = 「两份漂移」原病灶：blueprint/zones.json 声明的 spirit_qi 与玩家实际踩到的 raster qi_density 无契约关联（如 TSY 声明 -0.7 负压、raster 实测 0.93）。

## 接入面

- **进料**：`qi_field.build_qi_field`（统一场，SPIRIT_QI_TOTAL=100 预算口径，worldview §二/§十 守恒锚）；blueprint zones（grade/target_value）；各 profile 现有 qi_density 公式（迁移为对统一场基值的局部调制）
- **出料**：raster `qi_density.bin`（server mmap 消费：karma heat / HUD / agent world model）；manifest `qi_density_source` 置回 `"qi_field"`；raster_check 同源断言自动激活
- **共享类型**：`LayerSpec`（LAYER_REGISTRY）、`qi_density_from_field`、`QI_DENSITY_TOLERANCE`（qi_field.py ↔ raster_check.py 双端常量）
- **跨仓库契约**：server 读 raster 语义不变（值域 [-1,1] 已在 fix 分支放宽并有 pin 测试）；agent world model 的 qi_density 语义同步核对
- **worldview 锚点**：§二 灵气稀薄末法基调、§十 全服灵气守恒；负灵域（wangyintai/TSY 负压）正典

## 阶段划分（草案）

- ⬜ P0 — 烘焙管线：bake 期以统一场为 qi_density 基值写层（`qi_density_from_field(field)`），profile 手搓改为「对基值的有界调制」（Δ 上限待定，如 ±0.15），负灵域 profile（wangyintai/TSY 负压系）声明负 target 由场生成
- ⬜ P1 — profile 迁移：20+ profile 逐个迁移（每个 profile 的灵气叙事意图记入迁移对照表，防止细节丢失）；`qi_density_source` 置回 `"qi_field"`
- ⬜ P2 — 断言收紧与全图验证：全图 regen + validate 全绿；QI_DENSITY_TOLERANCE 复核（吸收调制 Δ + 采样噪声）；dev-reload.sh 闭环恢复
- ⬜ P3 — 下游语义核对：server karma heat / HUD 灵气显示 / agent world model 对新值域分布的消费复核；TSY 负压区玩家体验对拍（bot e2e 场景 `terrain_qi_semantic_*`）

## 开放问题

1. profile 调制 Δ 上限取多少才既保细节又不破同源断言（tol=0.2）？
2. TSY blueprint 声明 spirit_qi=-1.1 越 [-1,1] 域——blueprint 数据修正还是域扩展？
3. 灵脉（vein_network_field）在烘焙序里的位置：基值前还是调制后？
