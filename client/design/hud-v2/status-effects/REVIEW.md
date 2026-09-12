# 状态效果栏：第三轮交付

## 实现

`StatusEffectStore` 持有 `StatusEffectTimeline`，断线随既有会话清理入口清空。
完整服务端快照立即写入；只有 HUD 演出排队。同批按效果类别决定初始顺序，之后续期、
减层和服务器数组重排均不改变存续图标顺序。同 ID 丹药效果汇总层数、显示最长剩余时间。

单项入场 900ms（前 340ms 停留大图），随后下一项入场。占位权重渐变让整栏左右对称重排。
最多 8 格；窄窗口限制新入场数量，等待数量显示为 +N。最后 5 秒图标与框持续闪烁，
到期 280ms 退场；排队或正在入场时被清除直接取消。新快照校正本地剩余时间。

SVG 只提供图框、类别描边、流血渗出、丹毒扩散与未知状态标记；效果主体使用已有专属 PNG。
新生成定身、盾牌格挡、养伤、虚脱 4 张 PNG。使用本地配置中的 gpt-image-2，配置和密钥不入库。

虚脱图标在 2026-09-10 重试成功，已补齐 42 个基础效果的专属 PNG。
未知效果仍使用 SVG 降级标记。
提示词保存在 `scripts/images/gen_status_effects.py` 及 ignored `local_images/effects/`。

## 视觉证据

- `review-contact-sheet.png`：第一轮与第二轮、宽/窄窗口、5 个阶段的顶部等比例截图。
- `final-contact-sheet.png`：第二轮与第三轮对照，确认移除示例面板、修炼倍率及其预留空行。
- `png-contact-sheet.png`：4 张新 PNG 在深/浅背景及 24px 尺寸的对照。
- `round-1/`：854x480 framebuffer；首版 path 不被生产 SVG 子集支持，作为失败对照保留。
- `round-2-wide/`：1280x720 framebuffer，GUI scale 3。
- `round-2-compact/`：640x480 framebuffer，GUI scale 2。
- `round-3-wide/`、`round-3-compact/`：最终版在同样宽/窄窗口各 5 个动画阶段的截图。

截图来自真实 Minecraft planner、SVG 后端和 PNG 绘制。由显式 `BONG_SVG_HUD_PREVIEW=1`
夹具重建固定时间线，不能替代真实服务端状态同步验收。日志确认 socket/rim/blood/taint
分别生成 44/21/22/36 个三角形；PNG alpha、非空像素与阶段差异另做检查。

实机固定测试入口：`/scene test_status_effect_animation_1`。
仍受 `BONG_TEST_ENV=true` 与管理员权限限制，需要启动包含新增场景的服务端。

## 验证取舍

旧测试逐项绑定矩形数量、14px 填色、倒计时硬编码颜色，已由排队/取消/续期/过期、
居中、会话清理、同 ID 药效合层契约替代。保留真实 PNG 解码和生产 SVG 解析测试，
防止静态绘制命令合法而实机资源加载失败。生成工具增加 CDN 不转发密钥的回归测试。

按用户要求移除修炼倍率的客户端解析、存储、预览注入和 HUD 文字，状态图标仍正常展示。
会话清理测试删除已退役倍率字段的赋值与断言，保留效果快照和动画时间线的清理验证。
后端加速计算与状态同步保持原样。

三轮记录：首轮发现 path 不受生产解析器支持；第二轮改为受支持的几何并实机预览；
用户确认并要求移除示例和倍率文字后，第三轮收口显示、补齐虚脱图标、更新最终接触表。

## 本轮检查结果

- Java 17：`../scripts/build-token.sh gradle test build` 成功，5,142 项单测、3 项 GameTest 通过。
- Rust：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、完整 `cargo test`
  通过，均经 build-token 执行。主单测目标 11,980 项通过，其余集成与 doctest 目标均无失败。
- Python：图像生成工具 17 项测试通过；同步主线后的资源包构建 5 项测试通过。
- 最终实机预览：宽/窄窗口各 5 张截图，预览进程正常退出。
- 42 张 PNG 均通过 256x256、RGBA、透明背景及非空 alpha 检查。

## 验收边界

固定帧预览使用本地夹具。`/scene` 的权限、真实属性和快照同步由服务端行为测试覆盖，
本轮最终截图未通过新服务端 `/scene` 拍摄。图标技术检查不能代替人工美术判断。
