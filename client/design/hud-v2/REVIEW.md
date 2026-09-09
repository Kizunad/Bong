# HUD 第二轮视觉提案

本目录保留设计稿、生成工具和人工对照，不加入 Gradle main source set。
已认可的快捷栏槽框与施法残环由 `export_runtime.py` 导出到生产资源目录；
MiniBody 使用已认可的 PNG 人体与 A 方案材质条。下面的早期预览记录保留作对照。
第二轮已收到人工反馈：人体改为黄色剪影，逐项补全伤势枚举；其他造型获认可。
`round-2-wounds/` 是本次 MiniBody 修订，仍是独立预览，不代表最终游戏内视觉验收。

## 范围

| 部件 | 视觉方向 | 当前状态 |
|---|---|---|
| MiniBody | 黄色人体、真元与体力材质条，无面板黑底 | 完好人体 PNG 与 A 方案已接线；六种伤势 SVG/PNG 为独立示意，生产伤势仍取真实部位状态画标记 |
| Quickbar | 炭墨器槽、缺角和细刻痕；选中时骨白描边、底部刻片 | 普通/选中 SVG 已接双排槽位，保留真实图标与冷却遮罩 |
| CastBar 替代 | 聚拢的残环；完成合印，打断错位断裂 | 已绑定 CastState，12 弧段渐显并收拢；完成/打断沿用 store 的 300ms 终态寿命 |
| EventStream | 移除右侧滚动事件列表 | 已移除 orchestrator 中的列表入口；击杀反馈和共享事件缓冲保留 |
| TargetInfo | 取消默认完整面板，按感知能力分离 | 已移除总览入口；独立四件套 SVG 原型 |

`TARGET_INFO` 层同时承载 NPC 交互记录、秘境 Boss 与死亡提示，不能按 layer 总开关一起删除。
`EVENT_STREAM` 层同时承载击杀反馈，不能通过整个 layer 的过滤实现滚动列表移除。
旧 planner 源码暂保留；本轮只有具体生产入口退出，没有宣称事件协议和存储被删除。

## 目标感知四件套

| 独立资产 | 显示内容 | 未来激活条件 |
|---|---|---|
| `target-health.svg` | 生命状态的脉息刻度 | 获得独立生命感知能力，并收到可披露的目标生命状态 |
| `target-name.svg` | 窄题签上的名称 | 获得独立识名能力，并收到服务端允许披露的名称 |
| `target-realm.svg` | 境界层叠印记 | 获得独立境界感知能力，并收到允许披露的境界信息 |
| `target-qi.svg` | 真元气息环 | 获得独立真元感知能力，并收到允许披露的真元信息 |

这是四种不同信息的独立表达，不是同一完整面板的四个换色皮肤。
解锁生命不得连带解锁名称、境界或真元。当前没有新增功法 ID、服务端解锁字段、
客户端解锁开关或生产接线。后续接线默认全部关闭；仅拥有能力不代表目标信息可披露，
还必须尊重匿名、遮蔽及服务端观测结果。未知信息不应以零值冒充已知值。
名称和境界文字继续走 Minecraft 字体系统，SVG 只承担视觉外形。

正典对照：`docs/worldview.md` §三 L65-L72 的境界感知，以及 L517-L519 的神识信息差。
本稿不改正典，也不据此自行设定新功法或解锁门槛。

## 两轮差异

Round 1 确立人体、器槽、残环、感知四种形状语言；Round 2 加入人体结构线与局部
脉点、器槽的刻痕及选中刻片、残环内缘提示和感知部件的独立识别细节。
接触表两列使用相同尺寸、底色、状态和取景。目标名称与境界文字是排版示意。

## MiniBody 伤势修订

按本次人工反馈去掉人体内部骨线、肋线和脉点，完整人体只用 `#e8c64d` 黄色。
底衬、刻线、真元与体力刻度复用原稿的生成函数；其他 HUD 的 SVG 不改动。
全部伤势用同一左前臂示范，正面图中位于画面右侧，保持相同取景和尺度。

| WoundLevel | SVG | 视觉标记 |
|---|---|---|
| `INTACT` 完好 | `mini-body-intact.svg` | 纯黄色、完整轮廓 |
| `BRUISE` 淤伤 | `mini-body-bruise.svg` | 局部暗斑 |
| `ABRASION` 擦伤 | `mini-body-abrasion.svg` | 多道浅表擦痕 |
| `LACERATION` 割裂 | `mini-body-laceration.svg` | 一道深色开口 |
| `FRACTURE` 骨折 | `mini-body-fracture.svg` | 局部变色与折线断纹 |
| `SEVERED` 断肢 | `mini-body-severed.svg` | 前臂与手缺失，断端着色 |

枚举来源为 `com.bong.client.inventory.model.WoundLevel`。没有改动枚举颜色、功能系数
或其他业务语义；出血、愈合、夹板是 `BodyPartState` 的独立字段，本表未叠加。
断肢稿通过省略远端轮廓表达缺失，不用背景色盖住完整肢体。

对照表含上一稿与黄色剪影的同尺寸对比、六种完整 HUD、同位置局部放大和
56 × 74 px 等比例缩小图。缩小图用于辨识度评审，不冒充当前游戏内截图。
这六份是伤势的代表性视觉样本；生产接线仍需结合 16 个部位和实际 BodyPlan 投影。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 "client/design/hud-v2/make_wound_assets.py"
# 先按下节命令编译 RenderAssets.java，再指定要渲染的预览目录。
"/usr/lib/jvm/java-17-openjdk-amd64/bin/java" -Djava.awt.headless=true \
  -cp "/tmp/bong-hud-v2-classes:client/build/classes/java/main" \
  RenderAssets "client/design/hud-v2" "/tmp/bong-hud-v2-renders" "round-2" "round-2-wounds"
PYTHONDONTWRITEBYTECODE=1 python3 "client/design/hud-v2/make_wound_sheet.py" \
  "/tmp/bong-hud-v2-renders" "/tmp/bong-hud-v2-wounds-review"
```

本批 6 个 SVG 均通过生产 `NanoSvgParser` 和 `SvgTessellator`，最大 222 三角形，
非空且无越界；注入的 `filter` 仍被拒绝。Java2D 离屏网格预览的抗锯齿可能与游戏内不同。

### gpt-image-2 人体预览

后续人工指定：只用 `gpt-image-2` 重新生成 MiniBody，其他 HUD 保持现稿。
已复用主仓库 `scripts/images/.env` 与当前 `scripts/images/gen.py` 的 cliproxy 后端，
明确指定 `gpt-image-2`，没有修改凭据文件或切换模型。黄色剪影与现有纯黑水墨
HUD prefix 冲突，因此采用 `style=none` 的专用提示词。

本地产物位于被 Git 忽略的 `local_images/hud-minibody-gimage2/`：

- `round-1/mini-body-wounds.png`：1536 × 1024 的原始 RGBA 六状态图。
- `round-1/mini-body-wounds_prompt.md` 与 `generation.json`：提示词和非敏感生成参数。
- `preview/mini-body-<state>.png`：六张独立人体图，统一中轴、黄色和透明边缘。
- `preview/hud-<state>.png`：叠加原底衬及真元、体力刻度的组合示意。
- `preview/00-mini-body-contact-sheet.png`：中文标签、同部位放大和 HUD 组合对照。

原图有真实透明通道；后处理保留模型生成的轮廓和伤痕，没有套用会把黄色变黑的
水墨去白雾算法。完好图无伤色标记，五种伤势的伤色均局限于画面右侧前臂，断肢图
对应远端为空。当前仅为 PNG 视觉候选，尚未矢量化、接入游戏或提交生成物。

### 人体认可后的右侧状态条

人工认可 `gpt-image-2` 人体后，要求只调整右侧状态条的风格。两版组合共用同一
人体 PNG、同一底衬与位置，仍只表示真元和体力：

- A：`gpt-image-2` 生成玉色液柱与暖褐纤维材质；裁出两根独立材质，再按实际示例比例
  改变亮色填充高度，暗轮廓保留全容量。状态变化未另行生图。
- B：`make_body_meters.py` 生成收窄的连续 SVG 色带，真元用柔和反光，体力用纵向细纹。
  资产位于 `round-2-meters/`，两个 meter 独立；包含 full / normal / low / empty。

共同样本：满量 `1 / 1`、常态 `.64 / .82`、低量 `.12 / .18`、耗尽 `0 / 0`。
组合预览位于 `local_images/hud-minibody-meters/preview/00-meter-comparison.png`；同目录
有各状态独立组合 PNG，包含已认可的六种伤势。原始生图及提示词保留在其 `round-1/`。

验证：A 原图具有真实 alpha；两版的 8 个容量状态与对照稿左侧人体/底衬像素一致，
亮色区域随容量单调增加。B 的 9 个 SVG（含共用底衬）均通过生产解析、三角化和
边界检查，单文件最大 650 三角形，不支持的 filter 注入仍被拒绝。两版都是离屏
候选稿，其他 HUD 造型未改。

### A 方案接线状态

`MiniBodyHudPlanner.appendBar()` 已使用两张生产 PNG 材质边框：

- `assets/bong-client/textures/gui/hud/mini_body_qi_frame.png`
- `assets/bong-client/textures/gui/hud/mini_body_stamina_frame.png`

两张图只承载玉色液柱/暖褐纤维的窄侧壁和暗槽，中央余量仍由 planner 按实时比例
绘制，因此不会把静态满量图误当作动态状态。原有低量闪烁、季节真元色和耗尽不画
填充行为继续保留；人体伤势和其他 HUD 不变。

### SVG 生产接线

物品快捷槽当前保留主线入口，仅开放前 2 格（F1/F2），HUD 独立居中，背包绑定界面
和客户端按键、服务端绑定/使用校验同步限制。后续扩展来源确定为背包与装备，最多
10 格；本轮不新增装备属性或解锁协议。
下排 1-9 物品/技能栏默认也开放 2 格；后续扩展来源为身体条件（例如手部数量）和功法，
与上排背包/装备扩展独立。本轮只落实默认容量和入口校验，不新增扩容功法或身体计算规则。
两排分别按开放数量计算宽度并居中，增至奇数或偶数格时均围绕屏幕中线排列；侧边武器槽
和状态提示按较宽一排避让。按最新反馈，不考虑旧九格数据兼容：客户端、服务端绑定、
实体物品栏、存档、schema 与样例统一使用两格，不再区分存储容量与开放数量。
背包只显示两格，数字键 3-9 不切入隐藏原版槽，越界绑定/使用/搬入请求被拒绝。
`InspectScreen` 的物品槽数组、初始化、回执刷新和鼠标命中均使用同一开放数量；
拖拽解绑、失败回源与待确认回执继续走现有原子流程，未开放槽在发包前被拒绝。

`QuickBarHudPlanner` 通过 layer 内资产键选择普通或选中槽框；图标之后再绘制
冷却遮罩，遮罩收在图标范围内。施法高亮按 CastState 的 QUICK_SLOT / SKILL_BAR
来源定位，避免上下两排同索引一起亮。`CastRingHudPlanner` 复用原施法状态与进度，
放在快捷栏右上方，替代旧的槽下横条。

`HudRenderRegistry` 持有资源路径，planner 只输出资产键和目标矩形。
`SvgMesh` 保留 SVG 原始画布尺寸，后端据此缩放，避免 48×52 和 88×88 的设计稿
按原 1×1 矩形假设绘制而越界。SVG 与 Minecraft GUI 仍按命令顺序提交。

运行 `PYTHONDONTWRITEBYTECODE=1 python3 "client/design/hud-v2/export_runtime.py"`
可从第二轮原稿再生成 17 份生产 SVG。`SvgHudPreviewHarness` 仅在显式预览环境下
提供快捷栏及四种施法状态，施法夹具走本地预测入口，不伪造服务端 accepted 回执。

### 必要测试整理

- 容量改为两格后，移除旧绑定隐藏测试；越界请求由输入和协议门禁覆盖。旧测试中使用
  第三格以后的任意夹具索引迁到有效槽；冷却边界改成同一槽的表驱动测试，保留原业务断言。
- 保留来源行高亮、图标/遮罩顺序、施法状态转换与结束消失、SVG 资源选择和可见性测试。
- MiniBody 加宽后，fallback 也按原 30×75 源坐标投影；保留布局加载前后伤势位置一致性、
  HUD 锚点优先和缺省主锚点投影测试。移除重复的旧像素 spot check，将主锚点逐部位
  硬编码断言合并为中轴/非中轴两个代表，避免把显示尺寸当作固定业务契约。
- 连接标记移除后的旧测试改为验证区域标题、超载和 toast 的存活/过期，不再依赖该标记
  占据首条命令或以绘制调用数推断标题存在。

## 验证方式

从仓库根运行：

```bash
python3 "client/design/hud-v2/make_assets.py"
mkdir -p "/tmp/bong-hud-v2-classes"
"/usr/lib/jvm/java-17-openjdk-amd64/bin/javac" --release 17 -proc:none \
  -cp "client/build/classes/java/main" -d "/tmp/bong-hud-v2-classes" \
  "client/design/hud-v2/RenderAssets.java"
"/usr/lib/jvm/java-17-openjdk-amd64/bin/java" -Djava.awt.headless=true \
  -cp "/tmp/bong-hud-v2-classes:client/build/classes/java/main" \
  RenderAssets "client/design/hud-v2" "/tmp/bong-hud-v2-renders"
python3 "client/design/hud-v2/make_contact_sheet.py" \
  "/tmp/bong-hud-v2-renders" "/tmp/bong-hud-v2-review"
```

SVG 只使用当前 `NanoSvgParser` 支持的 polygon/ellipse 图元；图元解析和三角化直接
使用生产代码，离屏 PNG 使用 Java2D 绘制这些三角形。24 个文件均可解析、非空且
无越界，最大 352 三角形。注入 filter 后解析器拒绝，证明白名单检查有效。
离屏预览不等于 Minecraft 截图；Java2D 抗锯齿和 Minecraft 实际结果可能不同。

生产入口改动通过 Java 17 `scripts/build-token.sh gradle test build --offline --console=plain`：
5148 项 JUnit 和 3 项 GameTest 通过。新增测试在有效战斗 HUD 和非空目标/事件状态下
验证停用入口，避免因 HUD 全局隐藏而产生假绿。

### SVG 接线与两格快捷槽验证（2026-09-08）

- Java 17 `scripts/build-token.sh gradle test build --offline --console=plain`：
  5153 项 JUnit、3 项 GameTest 通过，构建成功。
  日志：`/tmp/bong-hud-two-slots-client-final.log`。
- 服务端 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过；
  `cargo test` 最终退出码 0，37 组共 12543 项通过、6 项忽略。
  日志：`/tmp/bong-quickslots-server-check.log`、
  `/tmp/bong-quickslots-server-test-confirm.log`。
- Bot `network_request_out_of_range` 通过；临时真实请求探针证明两格均可绑定/使用、
  第三格绑定拒绝且不改快照、未开放槽/空槽不打断正在进行的施法、v1 快照仍为九格。
  日志：`/tmp/bong-hud-two-slots-probe.log`。
- `network_quickslot_config` 整场未通过：本地 debug 服务端约 11-12 TPS，30 tick
  冷却不能用现实时间 1500ms 精确判定；日志中 1500ms 施法实际约 2.4s。
  同账号复跑另因上次保留的绑定不满足新角色前置而停止。两次失败均保留，未改宽计时
  断言，也不以临时探针冒充整场通过。日志：`/tmp/bong-hud-two-slots-bot.log`、
  `/tmp/bong-hud-two-slots-bot-confirm.log`。
- 快捷栏、聚环、收拢、完成、打断共五种真实 Minecraft 截图，分别保存于
  `/tmp/bong-hud-art-screenshots/`（3428x1377）和
  `/tmp/bong-hud-art-screenshots-small/`（640x480）。资源加载及非空像素检查通过；
  当前工具未可靠显示图片，尚不宣称人工检查过全部遮挡与视觉细节。
- 普通客户端使用 `runClient` 连接 `127.0.0.1:25566`，未启用 HUD/屏幕预览夹具；
  已确认新 SVG 槽框加载。世界画面和按 E 后截图分别为
  `/tmp/bong-hud-normal-game.png`、`/tmp/bong-hud-normal-inspect.png`。
  临时服务端数据库与 Redis 独立，皮肤使用项目已有的本地回退开关。

## 人工反馈后的工作

- 本次 MiniBody 伤势对照供人工评审；其他造型沿用已认可的第二轮，不宣称完成游戏内终验。
- MiniBody 的人体不是通用固定人形：生产接线需要尊重 `BodyPlan` 的异体部位与锚点，伤痕须取真实状态。
- Quickbar 接回真实物品/技能 PNG、冷却和选中态，验证双排槽位及武器侧槽遮挡。
- 残环绑定已有 `CastState`，确定与准星、截脉环互不遮挡的位置；低 GUI 尺寸下仍能分辨完成与打断。
- 四个目标感知部件保持独立原型，待独立能力设计与授权协议明确后再接入生产。
- 用真实 Minecraft 截图验证明暗场景、GUI scale、小尺寸、动画和 SVG/文字交错顺序。
