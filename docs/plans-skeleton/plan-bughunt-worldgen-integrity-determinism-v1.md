# BugHunt Worldgen Integrity & Determinism v1（骨架）

> 计划名：`plan-bughunt-worldgen-integrity-determinism-v1`
>
> 状态：`docs/plans-skeleton/` 骨架；本文只声明已登记为 skeleton，不追溯声称创建时已完成既有 promotion gate 的历史前置核验。尚未进入 active plan，不参与 `/consume-plan`。下一生命周期转换只能是 skeleton → active；转 active 前须将开放问题收口为 `§N.1 决议`，并重新核对 promotion gate 所需的仓库流程文件证据；证据不可读时保持 `BLOCKED`。
>
> Registry note：本文件进入仓库 registry 的事实是 `docs/plans-skeleton/` 路径与 master §6.12 ownership row；既有 promotion gate 的创建前核验不在本文件中伪造为已完成证据。后续 skeleton → active promotion 必须重新核对当时提交树中实际存在且可读取的仓库流程文件；若所需文件缺失或证据不可读，promotion 保持 `BLOCKED`，不得以本段或外部/会话文档替代。
>
> 范围：worldgen 产物从 blueprint/profile、布局与结构导出，到 raster/manifest、预览与 server runtime consumer 的完整性、确定性和边界合同。
>
> 明确不拥有：`#1623`、`#1627`、`#1853`。这三个未解决的 structure-manifest runtime wiring issue 由仍在进行的 `docs/plan-bughunt-structure-manifest-loot-consumer-v1.md` 作为当前 implementation owner；该计划的证据记录 worldgen 顶层 `corpse_mounds` / `ascension_pits` 字段被 Rust `RasterManifest` 丢弃，本文只保留 cross-reference 和跨计划集成验收边界，不宣称其已归档或已完成。


## 阶段总览

| 阶段 | 主题 | Issue | 状态 |
|---|---|---|---|
| P0 | profile/default、layer shape、mask/threshold validation | #1576 #1653 #1656 #1705 #1884 | ⬜ |
| P1 | deterministic seed、layout rotation、结构 bounds/orientation/anchor | #1610 #1684 #1793 #1795 #1798 #1799 #1848 | ⬜ |
| P2 | terrain/POI boundary 与 Rust consumer 对齐 | #1614 #1796 | ⬜ |
| P3 | regen atomicity/reentrancy、console marker refresh、preview compositor 全宽修复与回归 | #1751 #1753 #1766 | ⬜ |
| P4 | 集成回归与结构 manifest 跨计划接缝 | umbrella-owned issue：无；cross-reference：#1623 #1627 #1853 | ⬜ |

阶段日期在立项并完成对应验收后填写，格式为 `✅ YYYY-MM-DD`；当前骨架不预填完成日期。

## 接入面合同

### 进料（Inputs）

- `worldgen/` 下 blueprint、terrain profile、zone/profile defaults、layer 配置和人工结构布局输入。
- `worldgen/scripts/terrain_gen/fields.py` 的 `LAYER_REGISTRY` 及其 `LayerSpec(safe_default, blend_mode, export_type)`，作为 terrain layer shape、默认值和导出类型的现有来源。
- `worldgen/scripts/` 中结构生成器、布局/旋转/anchor/footprint 输入，以及结构导出和预览脚本。
- `worldgen` raster exporter 输出的二进制层、spans、manifest 和相关 metadata。
- `scripts/preview/compose_grid.py`、worldgen console/regen 入口及其 marker/manifest 输入。
- server 的 `RasterManifest`、terrain provider、`ColumnSample`、biome/POI runtime consumer 对 raster 及 manifest 的字段需求。
- 现有 worldgen 计划与已归档实现作为 ownership 和兼容边界，而不是重新定义同名合同。

### 出料（Outputs）

- 通过 fail-closed 校验的 blueprint/profile/default/layer-shape/threshold/mask 配置。
- 可复现的结构布局和 deterministic asset output：相同输入、版本和 seed contract 产生相同结果。
- 具有明确 bounds、rotation、orientation、anchor、footprint 语义的结构/布局 manifest 或导出物。
- 对 terrain surface、POI 高程和 biome boundary 做出可核验空间分类的 raster/runtime 结果。
- 原子、可重入且 generation-consistent 的 regen 输出；console marker 与实际产物同步。
- preview compositor 的全宽输出与回归产物；P3 必须修复 `raster_surface` 的目标画布宽度，再锁定该修复及边界行为，且不凭 #1766 重新定义未经证实的跨 cell ownership/clipping 合同。
- server runtime consumer 对 manifest/raster 字段的显式兼容性测试和回归证据。

### 共享类型 / 事件（Shared types / events）

优先复用现有类型和注册表，禁止为本计划复制第二份语义：

- `LAYER_REGISTRY` / `LayerSpec`：layer 名称、safe default、blend mode、export type。
- server `RasterManifest`、terrain provider、`ColumnSample` 及既有 raster layer identifiers。
- worldgen 的既有 `LayoutSpec`、placement/structure manifest、POI marker/regen generation metadata（最终具体 symbol 以 P0 决策门前的只读核验为准）。
- 既有 zone/profile blueprint 类型和导出字段；新增字段必须先确认 server consumer 是否真正读取。
- regen 使用的 generation/version/manifest 状态若已存在，应扩展现有合同，不另造并行 generation event。
- 本计划不新增 qi ledger event，也不把 worldgen 静态层误当作动态 `QiTransfer`。

### Server ↔ Agent ↔ Client 契约

- **Server：参与。** 重点核验 `RasterManifest`、terrain provider、`ColumnSample`、biome/POI consumer 和 server-side preview/regen 读取边界；P2/P4 必须证明 exporter 输出能被 server 正确读取，而不是只通过 Python snapshot。
- **Agent：默认不参与运行时改动。** 本计划没有新的天道 narration、world model、Redis command 或 agent-owned worldgen schema。若调查发现 regen/preview 必须通过 agent IPC 才能完成闭环，必须先在开放问题中收口，再另行定义 schema 和 Redis key；不得在实施阶段自行猜测。
- **Client：默认不参与运行时改动。** 预览脚本的图像/网格输出不是 Fabric gameplay payload。若 P3 发现预览结果由 client screenshot 或 CustomPayload 消费，则必须先锁定已有 server/client symbol、payload shape 和截图验收，再扩大本计划边界；否则保持 client out of scope。
- 跨层验收以真实产物链为准：profile/blueprint → exporter/manifest/raster → server loader/consumer；不能只测孤立 Python helper，也不能用 mock 代替 server 解析合同。

### Worldview 锚点

- `docs/worldview.md §二`：末法环境的灵气稀薄、死域/负灵域与灵压差；worldgen profile 和 mask/threshold 修复不得凭常识增加新的灵气语义。
- `docs/worldview.md §四`：灵脉/灵眼及其空间表现；terrain/profile 的灵气相关 layer 只能映射已有世界观概念。
- `docs/worldview.md §十`：全服灵气总量恒定并缓慢衰减；静态 worldgen raster 不能借此自定义运行时衰减公式。
- `docs/worldview.md §十三`：既有区域、zone 命名、地理尺度和距离语义；布局/POI 修复不得新造冲突 zone 或尺度。
- 六境界等修炼名词若出现在 profile/threshold 配置中，必须沿用正典的“醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚”顺序，不能引入旧称。

### qi_physics 锚点

- 本计划只消费 `qi_physics` 已定义的静态灵气预算/层字段合同；不新增 `*_DECAY*`、`*_DRAIN*`、`*_ATTEN*`、`RHO`、`BETA` 或自定义半衰/衰减常数。
- worldgen 的 `qi_density` 或类似 raster layer 是空间输入/静态预算表示，不是玩家或 zone 余额的权威写入；不得直接修改 `cultivation.qi_current` 或 `zone.spirit_qi`。
- 若某项实现需要动态灵气转移、归还、吸收或时代衰减，必须转交现有 `qi_physics::ledger::QiTransfer`、`qi_release_to_zone`、`qi_excretion` 或 `WorldQiBudget` 合同，不能在本计划内旁路。
- acceptance 只验证导出值、范围、shape 和 server 消费一致性；守恒行为由既有 qi_physics 计划和其测试负责。

### 既有计划边界与非重复约束

- `docs/plan-bughunt-structure-manifest-loot-consumer-v1.md`：当前 implementation owner，负责 `corpse_mounds` / `ascension_pits` 两个 worldgen 顶层字段被 Rust `RasterManifest` 丢弃的 runtime wiring gap；本计划只验证必要的跨计划产物接缝，不修改其 owned issue。
- `docs/plan-bughunt-anomaly-raster-runtime-consumer-v1.md`：可能承接 `#1656` 的 anomaly threshold runtime validation 子项；是否转移必须经开放问题决议。
- `docs/plan-bughunt-raster-check-required-layers-v1.md`：可能承接 `#1653/#1705` 中属于统一 required-layer shape contract 的部分；本计划保留 profile/layout/semantic 其余边界，不能重复定义 checker。
- `docs/plan-bughunt-worldgen-raster-check-cli-noop-v1.md`：拥有 raster checker CLI 可执行性/no-op 入口问题；本计划可以调用其 checker，但不重修 CLI main 或退出码合同。
- `docs/plan-bughunt-worldgen-pipeline-root-cwd-v1.md`：拥有 pipeline cwd/import root contract；本计划不把 cwd 修复重新算作 profile/export integrity。
- `docs/plan-worldgen-raster-check-qidensity-fix-v1.md`：已有 qi_density checker 历史断言收敛范围；本计划只处理本簇 issue 仍缺失的 profile/default/shape/boundary 语义，并以具体 ownership 决议为准。
- `docs/finished_plans/plan-worldgen-v4.md`、`docs/finished_plans/plan-worldgen-snapshot-v1.md`：作为现有接口和预览验证参考，不修改 finished plan 以吞并新 issue。

## P0：Profile/default、layer shape、mask/threshold validation

**Issue：** `#1576 #1653 #1656 #1705 #1884`

**目标：** 建立 profile/blueprint 输入、layer shape、depth-tier threshold 和 mask 语义的单一可验证合同，使错误配置在 exporter/checker 入口 fail-closed，而不是在运行时 KeyError、恒假 mask 或静默广播/截断。

**覆盖边界：**

- `#1576` blueprint dimension default：确认 dimension 的权威输入和缺失值行为。
- `#1653`、`#1705` scalar `mofa_decay` layer shape：确认 scalar 与 raster/layer shape 的规范表示，阻止 profile 间隐式 shape 漂移。
- `#1656` unknown depth tier 的 `anomaly_threshold` 查表：unknown tier 必须在 validation 入口 fail-closed，返回包含 profile、字段名和 tier 值的稳定可定位错误；禁止泄漏原始 Python `KeyError`、静默 fallback 或交给未声明 consumer 兜底。最终 ownership 见开放问题。
- `#1884` `spring_marsh` island mask：修复或重新定义 waterline/height/mask 的矛盾条件，确保 mask 具有非空且符合 profile 语义的可测结果。

**涉及模块 / 路径：**

- `worldgen/` blueprint、profile 和 terrain generation 配置。
- `worldgen/scripts/terrain_gen/fields.py` 的 `LAYER_REGISTRY` / `LayerSpec`。
- profile/export/validation 脚本及 `scripts/terrain_gen/harness/raster_check.py` 相关校验入口。
- 若 `#1656` 进入 runtime consumer，涉及 anomaly profile 到 server threshold consumer 的现有 symbol；不得另造并行表。

**可核验交付物：**

1. 每个 umbrella-owned profile 输入都有明确 required/default/invalid 状态，且缺失/未知值不会静默采用与其它 profile 不同的隐式行为。
2. `mofa_decay` 等 layer 的 scalar、per-column、per-cell 形状经过统一规范化或显式拒绝；`LAYER_REGISTRY`、exporter 和 checker 对同一 shape 解释一致。
3. depth-tier threshold 表对所有合法 tier 有正向 case；unknown tier 必须在 validation 入口 fail-closed，并返回包含 profile、字段名和 tier 值的稳定可定位错误。无论最终由本计划还是 anomaly 计划实现，都禁止原始 Python `KeyError`、静默 fallback 或未声明 consumer 兜底；若移交，本文留下同一错误合同和验收证据。
4. `spring_marsh` island mask 至少覆盖满足业务语义的正向样本、边界样本和矛盾输入；不再以互相排斥条件造成恒假结果。
5. 由真实 profile 生成 raster/manifest，运行 checker，再由 server loader/consumer 解析；测试断言外部产物和值域，而非只断言私有 helper 调用次数。
6. 变更不会改写动态 qi 物理：涉及 `qi_density` 的测试使用现有常量/预算合同，不写字面总量或自定义衰减率。

**Acceptance criteria：**

- `#1576`：缺省/显式 dimension 的正反样本均生成相同的规范 manifest；非法或缺失且无授权 default 的输入以非零退出并包含字段名和 profile 名。
- `#1653/#1705`：至少覆盖 scalar、正确 raster shape、空/错 shape、跨 profile 对拍；exported layer 的 dtype、shape、safe default 和 server decode 结果一致。
- `#1656`：合法 depth tiers 全部命中；unknown tier 必须在 validation 入口 fail-closed，并返回包含 profile、字段名和 tier 值的稳定可定位错误。原始 Python `KeyError`、静默 fallback 和未声明 consumer 兜底均为验收失败；若 ownership 转交 anomaly 计划，该计划也必须实现并测试同一错误合同。
- `#1884`：mask 生成包含满足 contract 的 true sample、false sample 和 waterline/height 边界 sample；恒假回归测试失败。
- P0 真实 profile → raster export → validation → server parse/consumer 集成测试通过；所有失败分支有可定位错误信息。

## P1：Deterministic seed、layout rotation、结构 bounds/orientation/anchor

**Issue：** `#1610 #1684 #1793 #1795 #1798 #1799 #1848`

**目标：** 统一人工结构和布局的确定性、旋转、朝向、footprint/bounds 和 anchor 合同，确保同一输入可重生成，结构不会越界、倒向错误或互相重叠。

**覆盖边界：**

- `#1610` jiu_zong_ruin flora variant 不得由固定/未声明选择造成非预期布局。
- `#1793/#1795` 不得使用 Python 进程随机化 `hash(name)` 作为可复现资产 seed。
- `#1684` rotation orientation 必须与布局坐标系/结构朝向一致。
- `#1798` 结构写出不得越过声明的 structure bounds。
- `#1799` great_hall 入口必须符合中庭/建筑朝向合同。
- `#1848` dan_zong compound anchor 不得与同象限 bagua placement 重叠。

**涉及模块 / 路径：**

- `worldgen/scripts/` 中 jiu_zong、wangyintai、dan_zong、stele_bone_coffin、great_hall 等生成器。
- 既有 layout/placement manifest、`LayoutSpec`、rotation/anchor/footprint/bounds 相关 helper。
- NBT/结构导出和结构 dump/ASCII 平面投影验证工具。

**建筑与布局硬约束：**

- 人工建筑摆位必须 deterministic；不得以 density/noise 作为主摆位机制。
- 坐标必须以明确 POI/compound 中心、相对 anchor 和 footprint 表示；rotation 必须声明旋转中心及坐标系。
- 复杂建筑/布局按 Round 1 first cut、Round 2 自评、Round 3 final review 打磨；每轮需有结构 dump、渲染截图或 ASCII 平面投影证据。该草案不产生 commit，正式实施时适用 `<PROMISE>` 终轮担保规则。

**可核验交付物：**

1. 稳定 seed 算法、seed 输入组成和 versioning contract 被显式记录；相同输入在独立进程中生成相同 variant/结构。
2. rotation、orientation、anchor、footprint 和 bounds 的坐标合同被实现并由 validator 验证。
3. 结构生成器输出边界内、无非法坐标、无指定禁重叠；入口方向通过平面投影/渲染证据核验。
4. 结构 manifest 与实际导出坐标一致，server/raster consumer 若读取该 manifest 则使用同一坐标语义。

**Acceptance criteria：**

- `#1610`：同一个输入 seed/variant 集合不会在未声明配置变化时固定偏向单一 flora variant；测试覆盖候选为空、单候选、多候选和重复生成。
- `#1793/#1795`：在两个独立 Python 进程、不同 `PYTHONHASHSEED` 下，对相同 canonical inputs 生成相同 seed、结构坐标和 manifest；输入名称/版本变化按决议的 contract 产生可解释变化。
- `#1684`：四个旋转方向和非方形 footprint 至少各有一条坐标对拍；结构 orientation 与 layout orientation 不发生 90/180 度错位。
- `#1798`：结构 dump 中所有写入坐标均落在声明 bounds；负坐标、边界坐标和越界坐标各有测试。
- `#1799`：入口相对中庭的朝向有几何验收，反向入口 fixture 必须失败。
- `#1848`：compound anchor、bagua placement 和 footprint 经过 overlap validator；同象限重叠 fixture 必须失败，合法相邻布局通过。
- P1 生成结果经过三轮视觉/几何验证，且 Round 3 证据覆盖 seed、坐标 bounds、朝向与 anchor。

## P2：Terrain/POI boundary 与 Rust consumer 对齐

**Issue：** `#1614 #1796`

**目标：** 让 terrain surface、POI 高程和 biome 分类按实际空间样本覆盖边界，并使 Python exporter 输出与 Rust runtime consumer 的分类/取样合同一致。

**覆盖边界：**

- `#1614` spawn POI fallback height：消除不受 terrain surface 或明确 fallback hierarchy 约束的错误高度。
- `#1796` chunk center biome classification：不以单一 chunk center 代表整块空间，避免边界列漏覆盖。

**涉及模块 / 路径：**

- `worldgen/` terrain surface/height sample、spawn/POI placement 和 profile safe-y/fallback 输入。
- Rust terrain biome classifier、`ColumnSample`、terrain provider 及 chunk/column runtime consumer。
- raster/manifest 导出和 server 解析集成测试。

**可核验交付物：**

1. POI fallback height 的优先级、来源和值域被明确记录；terrain sample 缺失、边界和无效高度均有 fail-closed/可解释处理。
2. biome classification 使用与 contract 相符的 column/boundary coverage，不再仅按 chunk center 漏掉边界列。
3. Python 生成的 boundary fixtures 经 raster export 后由 Rust consumer 得到相同分类和高度。
4. 不改变现有 zone 命名、维度和 worldview 地理尺度。

**Acceptance criteria：**

- `#1614`：覆盖正常 surface sample、surface 缺失、fallback 命中、越界/非有限高度和 POI 在边界列的 case；输出高度满足明确的 terrain/POI contract，不能静默使用任意常数。
- `#1796`：chunk 四边界、角点、跨 biome 边界和内部一致区域均有 sample；边界列分类与逐列 oracle 对拍，旧的 center-only fixture 必须撞红。
- 至少一条真实 worldgen raster → Rust `ColumnSample`/biome/POI consumer 集成测试证明 wire/endianness/坐标原点和分类一致。
- P2 不新增 client/agent runtime 依赖；若消费结果需要 S2C，仅复用已存在 payload contract 并先记录决议。

## P3：Regen atomicity/reentrancy、console marker refresh、preview compositor 全宽修复与回归

**Issue：** `#1751 #1753 #1766`。

**目标：** 让 worldgen regen 在并发/重复触发时保持 generation 一致和产物原子可见，console marker 与实际新产物同步；同时修复 preview compositor 将 `raster_surface` 以单个 `CELL_W` 宽度贴入双宽画布的 half-width 缺陷，并以修复后的完整目标宽度锁定 preview grid 回归基线。

**覆盖边界：**

- `#1751` console regen 后必须刷新 POI markers，而不是保留旧 marker 状态。
- `#1753` regen reentrancy 和 manifest 非原子更新必须有明确事务边界。
- `#1766`：修复 `scripts/preview/compose_grid.py` 将 `raster_surface` 以 `CELL_W` 左半宽贴入 `CELL_W * 2` 画布的缺陷，使其先按完整目标画布宽度生成，再以像素断言锁定全宽输出；同时覆盖单 cell、跨 cell、cell-edge 和负/正方向 fixture，防止修复回退；不把该修复扩写成新的跨 cell ownership/clipping 实现。

**涉及模块 / 路径：**

- worldgen console/regen entrypoint、manifest writer、POI marker refresh。
- generation id、临时产物、lock/rename 或现有等价原子提交机制。
- `scripts/preview/compose_grid.py` 及 preview cell/grid geometry helpers。
- 如预览由 server/client 消费，接入面只按开放问题决议扩大。

**可核验交付物：**

1. regen 的并发、重复触发、失败中止和成功发布状态有明确 state/generation contract。
2. manifest、raster、POI markers 和 preview 引用同一成功 generation；半写产物不会被 consumer 看到。
3. console regen 成功后旧 marker 被替换/失效，新 marker 与新产物一致。
4. preview compositor 先将 `raster_surface` 缩放到最终画布的完整 `CELL_W * 2` 宽度，再以像素断言锁定全宽输出；对 half-width failure 及 cell-edge/边界输入保留明确回归断言，不引入未经证实的 split/clipping/ownership 实现合同。

**Acceptance criteria：**

- `#1753`：两个并发 regen、同一 regen 重入、生成中途失败、manifest 写入失败和重复提交均有测试；只允许一个完整 generation 对外可见，失败不会把 manifest 指向半成品。
- `#1751`：regen 前后 POI marker 快照对拍，旧 marker 不残留，新 marker 坐标/代数与新 manifest 一致；刷新失败必须 fail-closed 而非静默成功。
- `#1766` 回归与修复 pin：先修复 `scripts/preview/compose_grid.py`，使 `raster_surface` 按完整 `CELL_W * 2` 目标宽度缩放后再贴入 row 2；随后用像素 fixture 验证左右两半均来自源图而非初始化背景，并覆盖单 cell、跨 cell、cell-edge 和负/正方向输入。该 pin 只锁定实际全宽修复，不扩写新的跨 cell ownership/clipping 实现。
- P3 的实际 console/regen 入口必须跑一次真实 end-to-end 产物链；不能只调用孤立 writer。
- 如果 P3 发现 Redis、server、agent 或 client 参与，新增 wire contract 必须先完成开放问题决议和 schema/sample/consumer 对拍；否则以 worldgen-only 验收收口。

## P4：集成回归与 structure-manifest 跨计划接缝

**Umbrella-owned issue：** 无新增 owned issue。

**Cross-reference only：** `#1623 #1627 #1853`；当前 implementation owner 仍是上述 active structure-manifest plan，本节不重新分配 ownership，也不将其未完成的 runtime wiring 写成已归档完成。

**目标：** 在不重新夺取三个 structure-manifest issue ownership 的前提下，验证 P0-P3 的 worldgen 产物与 structure manifest/loot consumer 计划能够组成一致的 exporter → manifest → server consumer 全链路。

**覆盖边界：**

- 本计划不修改、重述或“关闭”`#1623/#1627/#1853` 的根因。
- structure-manifest 计划记录的是 `corpse_mounds` / `ascension_pits` 两个 worldgen 顶层字段由 Rust `RasterManifest` 丢弃、因而尚未进入 runtime/loot consumer；本计划不据此改变 header 已声明的 active owner，也不把该 gap 写成已修复的 loot reference 或 manifest kind 错误。
- 本计划只负责在已有/完成后的 structure-manifest 接口上做 cross-plan regression：worldgen 输出的 profile/layout/regen/preview 改动不能破坏 manifest generation、坐标、generation id 或 server loader 的既有合同。
**可核验交付物：**

1. 一份 cross-plan boundary matrix 记录三个 excluded issue 的既有接口输入、输出、依赖和验收入口；不在此处重列或分配 ownership。
2. 一条真实 integration fixture 从 worldgen profile/layout/export 进入 structure manifest，再进入 server loader/loot consumer；测试只在两个计划都具备接口后启用。
3. cross-plan contract 对 `RasterManifest`、structure/placement metadata、POI/marker generation 和坐标系做字段级对拍。
4. P0-P3 的 regression suite 能在 structure-manifest plan 变更后重新运行，且不通过复制一份 schema/loader 来掩盖漂移。
5. 若 excluded plan 尚未完成，P4 记录明确 blocked evidence，不将其伪装成通过。

**Acceptance criteria：**

- `#1623/#1627/#1853` 在本计划的 issue ledger 中标记为 `cross-reference only`，不得出现在 umbrella-owned 修复清单或独立实现 commit 的 scope 中。
- 以真实导出物验证：`corpse_mounds` / `ascension_pits` 字段名称、结构坐标、generation/version 和 server serde decode 一致；字段缺失、serde 丢弃或未知字段的错误路径可定位。
- structure-manifest plan 完成后，运行跨计划 integration test；若其仍未完成，验收结果为明确 `BLOCKED`，并列出依赖的具体 symbol/fixture。
- P4 不修改 `docs/finished_plans/`，也不把新问题回写到已归档 plan；新增发现走对应 active/skeleton owner。

## 开放问题（P0 决策门前需收口）

以下问题不能在实施中由 subagent 自行猜测。每项必须先完成只读代码调查，再形成 `§N.1 决议`，并同时给出：具体结论、实施方案、边界条件、`file:line + plan section` 双锚点。

### §1 `#1653/#1705` 的 ownership

是否 scalar `mofa_decay` shape 属于统一 required-layer checker/profile contract，因而转入 `plan-bughunt-raster-check-required-layers-v1`；还是保留在本 umbrella 的 P0 作为 profile/export semantic validation？需要逐点核对两个 issue 命中的 exporter、`LAYER_REGISTRY`、checker 和 Rust consumer。

### §2 `#1656` 与 anomaly raster plan 的边界

unknown depth tier 的 `anomaly_threshold` lookup 是否直接属于 `plan-bughunt-anomaly-raster-runtime-consumer-v1` 的 runtime threshold contract？若转入，需保留本 umbrella 对 profile 输入/导出完整性的接缝验收，并要求 anomaly 计划实现完全相同的 fail-closed 错误合同：错误包含 profile、字段名和 tier 值，禁止原始 Python `KeyError`、静默 fallback 或未声明 consumer 兜底；若不转入，P0 必须拥有该完整 validation path。

### §3 Layer shape 的 canonical representation

scalar、per-column、per-cell、chunk 或 profile-wide layer shape 的规范表示由谁负责：profile generator、exporter normalization，还是 `raster_check.py` fail-closed validation？必须确定单一真相源、错误发生层和 server decode 对拍方式。

### §4 `#1884` island mask 的业务语义

正确修复是改矛盾条件、调整 waterline/height 输入，还是补写明确的 island mask contract？必须先定义 island 的正向/负向样本、边界条件和与 `spring_marsh` profile 的世界观语义。

### §5 `#1576` dimension default 的权威来源

dimension 是否必须由 blueprint 显式提供，是否允许 profile default，或 loader 是否应对缺失值 fail-closed？需要确认现有 blueprint schema、profile loader、导出 manifest 和 server 读取点的责任边界。

### §6 `#1793/#1795` 的 deterministic hash 与 versioning

稳定 seed 应采用哪种 canonical serialization/hash 算法，输入是否包含 profile、结构名、variant、坐标和版本，seed algorithm 是否需要显式 version 字段？必须兼顾跨进程/跨平台复现和已产出资产的迁移边界；不得继续依赖 Python `hash()`。

### §7 Layout rotation center 与 footprint contract

rotation 的中心、整数坐标 rounding、非方形 footprint、anchor 是先旋转后平移还是反之，必须在哪个 manifest/validator 层锁定？需要为四个方向和边界 footprint 形成可读的坐标 oracle。

### §8 Bounds、orientation 与 anchor 的验证层

`#1798/#1799/#1848` 的 bounds/orientation/overlap 校验是在生成器、NBT exporter、placement manifest，还是 raster/server validator 中完成？要求错误尽早失败，同时保留 server consumer 可核验的最终产物证据。

### §9 `#1614` POI fallback height authority

fallback height 的优先级是 terrain surface sample、profile safe-y、zone/POI 固定值，还是其它既有 helper？必须规定缺失、非有限、越界和跨 chunk 边界的处理，禁止仅凭视觉结果选常数。

### §10 `#1796` biome boundary classification semantics

分类单位是每个 sample column、chunk 内所有列、boundary-aware majority/coverage，还是现有 Rust classifier 的其它语义？需要建立 Python oracle 与 Rust consumer 的相同边界 fixture，避免只修一端。

### §11 `#1751/#1753` regen atomicity/reentrancy protocol

原子发布应采用临时目录 + rename、锁、generation id、写入 journal，还是已有 pipeline/console transaction？必须定义并发请求、失败恢复、旧 generation 保留和 marker refresh 的顺序，且不引入与已有 server/worldgen event 平行的生命周期。

### §12 `#1766` preview compositor 修复与回归边界

`#1766` 的已核实根因是 compositor 将 `raster_surface` 保持为单个 `CELL_W` 后贴入双宽画布，导致 row 2 右半边保持背景色。P3 必须先在 `scripts/preview/compose_grid.py` 将该源图缩放到完整 `CELL_W * 2` 目标宽度，再锁定左右两半像素均来自源图的回归；单 cell、跨 cell、cell-edge 和负/正方向只作为输入 fixture，不引入 split geometry、per-cell clipping 或 owner-cell 等新合同。若未来出现独立的跨 cell geometry 缺陷，必须以新证据另行立项。

### §13 P4 与 excluded issue 的 cross-plan 验收边界

structure-manifest plan 的完成信号、可用 fixture、server loader symbol 和 loot consumer symbol 是什么？在该计划未完成时，P4 应如何记录 `BLOCKED`，完成后由谁触发跨计划回归，必须明确而不能把三项 issue 偷渡为本计划 owned work。

### §14 是否需要 Agent/Client wire contract

P0-P3 当前预期是 worldgen/server 逻辑；需确认 console/regen/preview 是否实际通过 Redis agent IPC、CustomPayload 或 Fabric screenshot harness 才能构成用户可见闭环。若不参与，header 中保持明确 out-of-scope；若参与，先定义 schema/sample/payload/consumer 和对应跨栈 acceptance。

### §15 计划与已有 worldgen checker/preview 计划的最终 ownership matrix

需把 pipeline cwd、CLI no-op、required layers、qidensity、anomaly runtime、structure manifest、finished snapshot 与本 umbrella 的每个 issue/phase 做一对一映射，确认不存在重复修复、遗漏或“只引用但无人拥有”的孤岛。

## Skeleton → Active promotion gate

本文件已经是 master §6.12 登记的 Worldgen skeleton，不再经过 draft → skeleton 转换。后续 promotion 只能将其从 `docs/plans-skeleton/` 移到 `docs/plan-*.md`；在此之前：

1. 重新核对当前提交树实际存在且可读取的仓库流程文件，并保留对应读取证据；若 promotion 所需文件缺失或不可读，promotion 保持 `BLOCKED`，不得以外部或会话文档替代。
2. 为 §1–§15 开放问题补齐只读调查、`§N.1 决议` 和 `file:line + plan section` 双锚点；#1766 先完成 compositor 全宽修复，再以像素回归锁定该修复，不重新打开 geometry redesign。
3. 锁定 P0–P4 的最终 owner、共享类型、server consumer 和测试入口；implementation owner 固定为 `worldgen`，未决项不得进入依赖它的实现阶段。
4. 确认 `#1623/#1627/#1853` 仍由 active structure-manifest plan 负责，本文仅在 P4 做 cross-reference；该 runtime wiring 未完成前必须记录 `BLOCKED`。
5. active promotion 时更新阶段状态表和当前 `file:line` 锚点；`## Finish Evidence` 仅在全部阶段验收完成、归档前填写。promotion 前本 skeleton 不由 `/consume-plan` 消费，source issue 也不得仅凭 skeleton 登记就宣称已修复。

## §10 实施工作流

本计划 scope 覆盖 P0–P4，按单计划多 PR 串行消费；前一 PR 合入并完成验收后，才进入下一 PR。skeleton 阶段不执行以下实施步骤，亦不以本节替代 `docs/CLAUDE.md` §五的 `§N.1 决议` 门。

### §10.1 资产与布局变更的三轮打磨

P1 涉及 NBT、layout、placement 或复杂预览资产时，必须按 Round 1 first cut、Round 2 结构 dump/渲染/ASCII 自评、Round 3 终轮一致性复核执行；对应提交按 `(round 1/3)`、`(round 2/3)`、`(round 3/3)` 标记，终轮提交附 `<PROMISE>`，并覆盖 seed、bounds、orientation、anchor 和 footprint 证据。纯校验逻辑不适用该资产打磨门。

### §10.2 PR 序列与职责边界

1. **PR-1（P0）**：profile/default、layer shape、depth-tier unknown 输入和 island mask 的 canonical validation 与 pin/integration fixture。
2. **PR-2（P1）**：deterministic seed、layout rotation、bounds/orientation/anchor 及结构验证器。
3. **PR-3（P2）**：terrain/POI boundary、biome classification 与 Python exporter → Rust consumer 对拍。
4. **PR-4（P3）**：regen generation/原子发布、marker refresh、preview compositor 全宽修复与回归 pin。
5. **PR-5（P4）**：与 structure-manifest plan 的真实跨计划 integration fixture；依赖未完成时只提交明确 `BLOCKED` 证据。

每个 PR 只修改本阶段实际 owner 的代码、测试和本 active plan；不得修改 `docs/worldview.md`、`docs/finished_plans/` 或 excluded issue 的 owner plan。

### §10.3 独立实施与审查闭环

每个 PR 由独立实施 agent 按本节、对应阶段交付物和测试合同执行，完成后由无上下文只读 validator 对待审 HEAD 做第一性核验；validator 必须回报 HEAD 对拍和 PASS/FAIL。FAIL 时针对新 HEAD 返工并重新验证；任何合并主线造成 HEAD 变化也必须重新验证。实施、validator 和 review 结果不得以孤立 Python 单测替代真实 exporter/manifest/server consumer 链路。

### §10.4 本地门禁、主线同步与 review 等待

按受影响栈执行对应完整门禁；worldgen/server 跨栈变更同时跑两端门禁。推送前紧邻执行 `git fetch origin && git merge origin/main`，若 merge 带入受影响变更则重跑门禁和 validator。开 PR 后使用仓库规定的独立 review 入口和 CodeRabbit 检查；检查 pending 时按 `docs/CLAUDE.md` §六的 `ScheduleWakeup` 等待协议，不以本地自判替代复审。

### §10.5 单次 consume-plan 收口

用户提交 `/consume-plan` 后，消费流程按 PR-1→PR-5 串行推进：每个 PR 完成实现、validator、栈门禁、主线同步、review 收敛和合入后再开下一 PR；全部 P 阶段完成后更新状态、补 `## Finish Evidence`，最后将 active plan 迁入 `docs/finished_plans/`。任一 excluded plan 或上游合同未完成时保留可核验 `BLOCKED` 证据，不得把未实施 issue 宣称为完成。
