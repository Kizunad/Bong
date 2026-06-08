# Bong · plan-resourcepack-v1 · active

**资源包统一交付**——将 `plan-mineral-v2` P5 实装的单包（`bong-mineral-v1.zip`）扩展为**全量 Bong 内容统一包**（`bong-full-vN.zip`），覆盖矿物 / 实体模型 / 音效 / VFX 贴图等所有非 vanilla 资产，通过 Valence `ResourcePackPrompt` 在玩家 join 时自动推送，并建立 CI 自动构建 + sha1 校验 + 版本号管理流水线。

**来源**：`plan-mineral-v2.md` P5 §6 "待立 `plan-resourcepack-v1`（P5 Valence `ResourcePackPrompt` 接入）"；早期架构备忘（`docs/scribble.md`）"热更新微端 + 资源包版本校验"

**交叉引用**：`plan-mineral-v2.md` ✅（已有 `bong-mineral-v1.zip` 基础 + `scripts/build-resourcepack.sh` 雏形 + `ResourcePackConfig` Resource）· `plan-entity-model-v1.md` ✅（BBModel / GeckoLib 模型资产，输入本 pack）· `plan-vfx-v1.md` ✅（VFX 粒子贴图资产）· `plan-audio-v1.md` ✅（audio_recipe 引用的 sound 文件）· `plan-audio-world-v1.md` ✅（世界音效文件）· `plan-model-asset-v1.md` ⬜ skeleton（Tripo3D 生成的 3D 物品模型）

**worldview 锚点**：无直接世界观锚（纯基础设施 plan）；服务世界观所有视听内容的交付质量——资源包不下发 → 玩家看不到自定义粒子/模型 → 所有 VFX/NPC 皮肤计划的视听规格失效。

**qi_physics 锚点**：不涉及真元/灵气计算。

**前置依赖**：
- `plan-mineral-v2` ✅ — 已有 `bong-mineral-v1.zip` + `build-resourcepack.sh` + `ResourcePackConfig` Resource + Valence ResourcePackPrompt 接线基础
- `plan-entity-model-v1` ✅ — 实体模型资产（输入 pack）
- `plan-vfx-v1` ✅ — VFX 贴图资产（输入 pack）

**反向被依赖**：
- 所有视听规格 plan（vfx / audio / entity-model / model-asset）——资产可被玩家客户端加载的前提
- 未来 `plan-deploy-v1`（如有）— 资源包托管和 CDN 分发

---

## 接入面 Checklist

- **进料**：
  - `client/resourcepack/` 各子目录（矿物、实体、VFX、音效）的原始资产文件
  - `scripts/build-resourcepack.sh`（mineral-v2 已有雏形，需扩展为 merge 多目录）
  - `server/src/network/connection.rs` 的 `ResourcePackConfig` Resource（mineral-v2 已有）
- **出料**：
  - `client/resourcepack/bong-full-vN.zip` — 合并包（含矿物 + 实体 + VFX + 音效）
  - `client/resourcepack/manifest.json` — 版本号 + sha1 + 子包列表
  - `server/src/network/` — join hook 推送逻辑（升级 mineral-v2 单包推送为 full pack）
  - CI: `.github/workflows/build-resourcepack.yml` — 自动构建 + 上传 Release artifact
- **共享类型**：扩展 `ResourcePackConfig { url, sha1, force_accept }` 为 `ResourcePackConfig { packs: Vec<PackEntry> }` 支持多包顺序加载（Minecraft 1.20.1 支持多 resource pack 叠加）
- **跨仓库契约**：server push `ResourcePackPrompt` 给 client（Valence 原生 packet）；client 无需修改（原版 Minecraft 客户端处理 resource pack 接受/拒绝）；agent 不涉及

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | 资产清单审计 + `manifest.json` 格式定稿 + `build-resourcepack.sh` 扩展为 merge 多子目录 | ⬜ | 单测：merge 脚本生成的 zip 包含 mineral / entity-model / vfx 贴图；sha1 与 manifest.json 一致 |
| **P1** | server join hook 升级（单包 → 支持 `packs` 列表；降级策略：client 拒绝 → 记录日志但不 kick）| ⬜ | 集成测试：mock client join → 收到 `ResourcePackPrompt` 含正确 URL + sha1；拒绝 pack → server 记录 `resource_pack_declined` 事件但不断线 |
| **P2** | CI 自动构建（GitHub Actions: PR merge → build zip → 上传 Release artifact → sha1 写入 `ResourcePackConfig`）| ⬜ | CI 流水线跑通；build artifact URL 可直接填入 `ResourcePackConfig.url` |
| **P3** | 版本号管理（`manifest.json` 含 `version: "vN"` + 玩家 session 记录已接受版本；版本变更时触发重推） | ⬜ | 单测：相同版本 join 不重推；版本升级后首次 join 触发重推 |

---

## §7 开放问题（P0 决策门前需收口）

1. **多包 vs 单合包**：Minecraft 1.20.1 支持最多 20 个 resource pack 叠加；是保持多个小包（矿物包 / 实体包分开推）还是合并为一个大包？——建议单合包（减少 HTTP 请求数，管理更简单；客户端加载顺序固定）
2. **托管方式**：GitHub Release artifact（免费但有带宽限制）/ 内网 NGINX / Cloudflare R2？——v1 先用 GitHub Release；生产时按实际带宽压力决定是否迁 CDN
3. **`force_accept` 策略**：拒绝资源包时 kick 还是降级 vanilla 贴图？——mineral-v2 已决策为"不 kick，降级"；本 plan 沿用此决策，记录 `resource_pack_declined` 事件供运营监控

---

## §7.1 决议参考（来自 plan-mineral-v2 §10 / §11）

plan-mineral-v2 P5 已决策：
- **方案 A**（`Valence::Client::set_resource_pack` join hook）已实装
- 拒绝 pack → **降级，不 kick**
- 现有 `bong-mineral-v1.zip` sha1 已验证（`scripts/build-resourcepack.sh`）
- **本 plan P0 以此为起点扩展**，不重做 mineral-v2 已完成部分
