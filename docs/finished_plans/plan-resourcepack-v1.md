# Bong · plan-resourcepack-v1 · finished

**资源包统一交付**——将 `plan-mineral-v2` P5 实装的单包（`bong-mineral-v1.zip`）扩展为**全量 Bong 内容统一包**（`bong-full-vN.zip`），覆盖矿物 / 实体模型 / 音效 / VFX 贴图等所有非 vanilla 资产，通过 Valence `ResourcePackPrompt` 在玩家 join 时自动推送，并建立 CI 自动构建 + sha1 校验 + 版本号管理流水线。

**来源**：`plan-mineral-v2.md` P5 §6 "待立 `plan-resourcepack-v1`（P5 Valence `ResourcePackPrompt` 接入）"；早期架构备忘（`docs/scribble.md`）"热更新微端 + 资源包版本校验"

**交叉引用**：`plan-mineral-v2.md` ✅（已有 `bong-mineral-v1.zip` 基础 + `scripts/build-resourcepack.sh` 雏形 + `ResourcePackConfig` Resource）· `plan-entity-model-v1.md` ✅（BBModel / GeckoLib 模型资产，输入本 pack）· `plan-vfx-v1.md` ✅（VFX 粒子贴图资产）· `plan-audio-v1.md` ✅（audio_recipe 引用的 sound 文件）· `plan-audio-world-v1.md` ✅（世界音效文件）· `plan-model-asset-v1.md` ✅ archived（2026-06-09 归档；仅 P-done 19 个 Tripo3D 模型可用，P0-P4 因会员到期作废）

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
| **P0** | 资产清单审计 + `manifest.json` 格式定稿 + `build-resourcepack.sh` 扩展为 merge 多子目录 | ✅ | 单测：merge 脚本生成的 zip 包含 mineral / entity-model / vfx 贴图；sha1 与 manifest.json 一致 |
| **P1** | server join hook 升级（单包 → 支持 `packs` 列表；降级策略：client 拒绝 → 记录日志但不 kick）| ✅ | 集成测试：mock client join → 收到 `ResourcePackPrompt` 含正确 URL + sha1；拒绝 pack → server 记录 `resource_pack_declined` 事件但不断线 |
| **P2** | CI 自动构建（GitHub Actions: PR merge → build zip → 上传 Release artifact → sha1 写入 `ResourcePackConfig`）| ✅ | CI 流水线跑通；build artifact URL 可直接填入 `ResourcePackConfig.url` |
| **P3** | 版本号管理（`manifest.json` 含 `version: "vN"` + 玩家 session 记录已接受版本；版本变更时触发重推） | ✅ | 单测：相同版本 join 不重推；版本升级后首次 join 触发重推 |

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

---

## Finish Evidence

### 落地清单

- **P0 全量资源包构建**：`scripts/build-resourcepack.sh` 将 `client/src/main/resources/assets/` 下矿物、实体模型、VFX、HUD effect、音频配方等运行时资源合并为 `bong-full-v1.zip`；`client/resourcepack/manifest.json` 固化 `version`、`sha1`、`size` 和子包 file count。
- **P0 构建测试**：`scripts/test_build_resourcepack.py` 覆盖完整构建、空资产树、白名单过滤、缺失 assets root 错误分支。
- **P1 server 推送**：`server/src/network/resourcepack.rs` 的 `ResourcePackConfig { packs }` 支持按序推送；`prompt_resource_pack_on_join` 在 join 后发送 Valence resource pack prompt；拒绝/下载失败只记录降级状态，不 kick。
- **P2 CI artifact**：`.github/workflows/build-resourcepack.yml` 在 PR 构建并上传 zip/sha1/manifest artifact；main push 发布 `resourcepack-v1` GitHub Release assets；workflow 校验生成 manifest 与 `ResourcePackConfig` 默认 URL/sha1 不漂移。
- **P3 版本管理**：`ResourcePackEntry.version` 与 `ResourcePackStatusStore` session 状态记录已成功加载的 `{version, sha1}`；同版本同 sha1 不重复推，版本或 sha1 变化会重新推送。

### 关键 commit / PR

- `e736d8b7f` / PR #446：实现资源包全量清单 P0。
- `890c66c73` / PR #447：实现资源包 server join hook P1。
- `83eacd0e4` / PR #448：实现资源包 CI artifact 与 Release 发布 P2。
- `f145a78eb`：实现资源包版本接受记录 P3。

### 测试结果

- `python3 -m unittest scripts/test_build_resourcepack.py`：4 tests passed。
- `bash -n scripts/build-resourcepack.sh`：通过。
- `BONG_RESOURCEPACK_OUT_DIR=$(mktemp -d) BONG_RESOURCEPACK_VERSION=v1 bash scripts/build-resourcepack.sh`：生成 `bong-full-v1.zip`，sha1=`9af0504a8f09b08d308d3d9f3cb5e9853f6dc0e3`。
- `cd server && cargo test resourcepack`：14 tests passed。
- `cd server && cargo fmt --check`：通过。
- `cd server && cargo clippy --all-targets -- -D warnings`：通过。
- GitHub Actions run `27140750320`：main `Build resource pack` 与 `Publish release asset` jobs 均 success；Release `resourcepack-v1` 已包含 `bong-full-v1.zip`、`bong-full-v1.zip.sha1`、`manifest.json`。

### 跨仓库核验

- **server**：`ResourcePackConfig` 默认 URL 指向 `https://github.com/Kizunad/Bong/releases/download/resourcepack-v1/bong-full-v1.zip`；默认 sha1 来自 manifest 常量；`ResourcePackStatusStore` 记录 session 级 accepted version。
- **client**：无需 Fabric client 代码接线；原版 Minecraft 客户端处理 Valence `ResourcePackSendS2c`，资源来自 `client/src/main/resources/assets/`。
- **agent**：不涉及 agent/schema 变更。

### 遗留 / 后续

- 生产带宽与 CDN 迁移不在本 plan 范围；当前 v1 托管在 GitHub Release。
- `plan-client-login-ux-v1` 可基于本 plan 的 manifest/version/sha1 结果继续做登录 UX 与缓存提示。
