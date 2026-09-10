# HUD v2 设计目录审查记录

本目录保留已经进入 SVG 生产导出链的设计稿与生成工具，**不加入 Gradle main source set**。
它是设计与导出证据目录，不是客户端运行时源码；运行时资源仍只位于
`client/src/main/resources/assets/bong-client/svg/hud/`。

## 当前生产链

`make_assets.py` 生成第二轮的四个接线稿：

- `round-2/quick-slot.svg`
- `round-2/quick-slot-selected.svg`
- `round-2/cast-complete.svg`
- `round-2/cast-interrupted.svg`

`export_runtime.py` 从这四个稿件拷贝快捷栏与施法终态资源到运行时目录，并使用
`Art` / `cast` 生成 `cast-track.svg` 与 `cast-segment-a.svg` 至
`cast-segment-l.svg`。这些文件由 `HudRenderRegistry`、`QuickBarHudPlanner` 和
`CastRingHudPlanner` 通过 `SvgHudBackend` 加载；图标、冷却遮罩、动态文字仍由
Minecraft GUI 绘制。

## 设计意图

快捷栏槽框使用炭墨器槽、缺角与细刻痕，选中态使用骨白描边和底部刻片。施法残环
按 `CastState` 区分进行、完成与打断，完成态收拢并合印，打断态错位断裂。SVG 只
提供静态外形，实际状态、进度、图标和文字仍由客户端 HUD planner 负责。

`cast-form` 与 `cast-gather` 不是生产资源：客户端只在显式启用的
`SvgHudPreviewHarness` 中把场景名映射到本地预测夹具，没有从这两个设计稿加载 SVG。
目标感知四件套、MiniBody 人体/伤势样本和状态条候选也未进入客户端运行时，已从本目录
移除，待未来具备真实能力授权、部位投影和运行时接线后另行设计。

`example.svg` 仍是 `SvgHudBackend` 的显式预览资源，`primitive-rect.svg` 仍由
`HudRenderRegistry` 的既有 SVG frame surface 使用；它们不属于本次设计稿清理范围。
