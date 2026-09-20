# 末法命名禁词存量盘点

> 盘点日期：2026-09-19。本文是存量清单，不是改名决定；本次不修改任何
> `id`、`name`、`description`，也不回写世界观正典。

## 正典对照

`CLAUDE.md:254` 给出的落地规则是：

> **命名禁词**（worldview.md §三 L63 的命名原则落地速查）：末法时代禁用 玄/陨/星/仙/太/古；优选衰败素朴意象 残/碎/锈/杂/粗/髓/朴/枯。例外：已入世俗医药的矿名（丹砂/朱砂/雄黄）OK。

正典原文 `docs/worldview.md:63` 是：

> 不沿用传统仙侠那套境界称谓，太传统了。末法时代的修士不配用上古称呼。

对照结论：两者**不完全是同一句规则**。`docs/worldview.md:63` 是“禁用传统仙侠境界称谓/上古称呼”的原则，`CLAUDE.md:254` 将它操作化为六个字、推荐词和医药矿名例外。本文按 `CLAUDE.md:254` 做机械盘点，并把这层扩展记录下来；不据此修改 `docs/worldview.md`。

## 扫描与计数口径

- 扫描 `server/assets/**/*.toml` 的 `name` 和 `description` 字段，只按中文字 `玄/陨/星/仙/太/古` 命中；不扫拼音、不把注释当字段。
- 一行表格对应一个唯一的 `(文件, id)`；共 **42 条**，命中字段共 **56 行**。其中 `name` 命中 **15 条**，`description` 仅命中 **27 条**。`name` 的字符计数为：玄 7、陨 2、星 1、仙 0、太 0、古 5。
- “引用数”是在当前主线快照上用 `git grep -n -F` 扫 `server/`、`client/`、`docs/`（排除 `docs/finished_plans/`）以及 `modelScript/tests/fixtures/`、`proto/fixtures/`，再按完整 id 边界计数。它是匹配次数而不是文件数，包含资产定义行；归档 plan 不计入影响面。
- 本表的“建议改”只用于 `name` 字段明显命中且没有医药矿名豁免的记录；`description` 单独命中的记录统一标为“存疑”，因为命名禁词是否扩展到叙事文本需要用户拍板。

## 逐条清单：建议改（15 条 name 命中）

这些条目的名称直接含禁词，且没有“丹砂/朱砂/雄黄”一类的世俗医药矿名豁免。候选名只是降低裁决成本，不是本 PR 的决定。

| 文件与行 | id | 当前 name | 命中的字 / 证据 | 建议归类与候选 | git grep 引用数 |
|---|---|---|---|---|---:|
| `server/assets/items/anqi.toml:125,132` | `anqi_shanggu_bone` | 上古残骨 | 古；`name=上古残骨`；description 同样写“上古残骨” | **建议改**：候选“残骨”；与下一条区分为“封元残骨” | 5 |
| `server/assets/items/anqi.toml:137,144` | `anqi_shanggu_bone_charged` | 封元上古残骨 | 古；`name=封元上古残骨`；description 同样写“上古残骨” | **建议改**：候选“封元残骨” | 4 |
| `server/assets/items/botany_v2.toml:80` | `xuan_rong_tai` | 玄绒苔 | 玄；`name=玄绒苔` | **建议改**：候选“枯绒苔”或“残绒苔” | 10 |
| `server/assets/items/dandao.toml:26,33` | `dandao.ancient_recipe_fragment` | 上古丹方残页 | 古；`name=上古丹方残页`；description 同样写“上古丹宗” | **建议改**：候选“残丹方页” | 3 |
| `server/assets/items/forge.toml:33,40` | `xuan_iron_anvil` | 玄铁砧 | 玄；`name=玄铁砧`；description 同样写“玄铁” | **建议改**：候选“锈铁砧” | 3 |
| `server/assets/items/forge.toml:137,144` | `xuan_iron` | 玄铁 | 玄；`name=玄铁`；description 同样写“玄铁” | **建议改**：候选“锈铁” | 22 |
| `server/assets/items/lingtian.toml:31,38` | `hoe_xuantie` | 玄铁锄 | 玄；`name=玄铁锄`；description 同样写“玄铁” | **建议改**：候选“锈铁锄” | 24 |
| `server/assets/items/minerals.toml:109,116` | `gu_tong_pian` | 古铜片 | 古；`name=古铜片`；description 写“古铜片”与“上古礼器” | **建议改**：候选“锈铜片”或“旧铜片” | 20 |
| `server/assets/items/pills.toml:166,173` | `poison_pill_fu_xin_xuan_gui` | 腐心玄龟丹 | 玄；`name=腐心玄龟丹`；description 写“玄龟壳” | **建议改**：候选“腐心残龟丹” | 8 |
| `server/assets/items/pills.toml:224,231` | `poison_powder_fu_xin_xuan_gui` | 腐心玄龟粉 | 玄；`name=腐心玄龟粉`；description 写“玄龟丹” | **建议改**：候选“腐心残龟粉” | 2 |
| `server/assets/items/sword_materials.toml:19,26` | `meteor_iron` | 陨铁 | 陨；description 另写“古遗迹” | **建议改**：候选“坠铁” | 5 |
| `server/assets/items/sword_materials.toml:30,37` | `star_iron` | 星辰铁 | 星；description 写“古遗迹”“上古宗门” | **建议改**：候选“蓝铁”或“冷蓝铁” | 19 |
| `server/assets/items/sword_materials.toml:41,48` | `sky_meteor_iron` | 天外陨铁 | 陨；description 同样写“陨铁碎片” | **建议改**：候选“天坠铁” | 2 |
| `server/assets/items/sword_materials.toml:87,94` | `ancient_sword_embryo` | 上古剑胚 | 古；`name=上古剑胚`；description 同样写“上古剑胚” | **建议改**：候选“残剑胚” | 15 |
| `server/assets/items/weapons.toml:194,201` | `flying_sword_feixuan` | 飞玄剑 | 玄；description 同样写“玄铁” | **建议改**：候选“飞残剑” | 12 |

## 逐条清单：存疑（27 条 description-only 命中）

这些条目没有命中 `name`。其中有些是直接引用上面待裁决的材料名，有些只是“古怪/古战场/火星/太多”这类普通叙事或词内字；在没有用户决定“禁词是否约束 description”前，不提出替换名。

| 文件与行 | id | 当前 name | 命中的字 / description 证据 | 建议归类 | git grep 引用数 |
|---|---|---|---|---|---:|
| `server/assets/cultivation/techniques.toml:552` | `tuike.transfer_taint` | —（无 name） | 古；“化虚上古皮可吸永久标记” | **存疑**：效果描述中的材料语义，是否随名称规则收紧待定 | 12 |
| `server/assets/cultivation/techniques.toml:654` | `anqi.echo_fractal` | —（无 name） | 古；“上古残骨分形为多条真实 echo 弹道” | **存疑**：直接关联上古残骨资产，但不是名称字段 | 34 |
| `server/assets/cultivation/techniques.toml:671` | `body.guangbo_ticao` | —（无 name） | 古；“一套古怪的伸展动作” | **存疑**：普通形容词“古怪”，疑似命名扫描误报 | 62 |
| `server/assets/items/anqi.toml:108` | `anqi_fenglinghe_bone` | 封灵匣骨 | 古；“上古残骨碎片改制” | **存疑**：直接引用待裁决的上古残骨名称，随其联动 | 5 |
| `server/assets/items/armor.toml:331` | `armor_copper_helmet` | 铜甲盔 | 古；“古铜色轻盔” | **存疑**：颜色/材质形容，不是 name 命中 | 3 |
| `server/assets/items/armor.toml:353` | `armor_copper_leggings` | 铜甲腿甲 | 古；“古铜护腿” | **存疑**：颜色/材质形容，不是 name 命中 | 3 |
| `server/assets/items/body_scrolls.toml:12` | `scroll_body_guangbo_ticao` | 残卷·广播体操 | 古；“一套古怪的伸展动作” | **存疑**：普通形容词“古怪”，疑似命名扫描误报 | 5 |
| `server/assets/items/botany_v2.toml:32` | `duan_ji_ci` | 断戟刺 | 古；“古战场金属遗物” | **存疑**：世界背景地点描述，不是名称字段 | 7 |
| `server/assets/items/botany_v2.toml:43` | `xue_se_mai_cao` | 血色脉草 | 古；“古战场脉草” | **存疑**：世界背景地点描述，不是名称字段 | 16 |
| `server/assets/items/botany_v2.toml:175` | `ling_jing_xu` | 灵晶须 | 陨、古；“陨坑晶柱”“上古器物” | **存疑**：资源背景叙事，是否需要替换“陨坑/上古”待定 | 7 |
| `server/assets/items/coffin_tiers.toml:30` | `stone_coffin` | 乌石棺 | 玄；“玄铁骨架” | **存疑**：直接引用 `xuan_iron` 的旧名，随材料裁决联动 | 9 |
| `server/assets/items/coffin_tiers.toml:42` | `bronze_coffin` | 青铜棺 | 古；“上古饕餮纹”“古铜片” | **存疑**：直接引用 `gu_tong_pian`，并夹有背景叙事 | 9 |
| `server/assets/items/coffin_tiers.toml:57` | `scroll_jade_coffin` | 寒玉棺残卷 | 古；“古法” | **存疑**：普通历史叙事，是否属于命名规则待定 | 11 |
| `server/assets/items/coffin_tiers.toml:79` | `scroll_bronze_coffin` | 青铜棺残卷 | 古；“上古炼器文字” | **存疑**：背景叙事，不是名称字段 | 17 |
| `server/assets/items/core.toml:84` | `spirit_treasure_jizhaojing` | 寂照镜 | 古；“上古清风宗掌教” | **存疑**：传说背景专名语境，需用户决定是否改叙事 | 15 |
| `server/assets/items/core.toml:411` | `grass_pouch` | 小草包 | 太；“别指望装太多东西” | **存疑**：词语“太多”的普通副词用法，疑似扫描误报 | 24 |
| `server/assets/items/fauna.toml:156` | `jing_gu` | 苍鲸脊骨 | 古；“龙鳞古意” | **存疑**：普通审美描述，不是名称字段 | 8 |
| `server/assets/items/food.toml:104` | `food.container.ice_cellar` | 寒冰窖格 | 玄；“玄冰石” | **存疑**：材料称谓但未发现对应 name 字段，需先定材料词表 | 5 |
| `server/assets/items/forge.toml:54` | `dao_anvil` | 道砧 | 星；“火星” | **存疑**：普通名词“火星”，疑似扫描误报 | 3 |
| `server/assets/items/materials.toml:241` | `broken_artifact` | 破碎法宝 | 古；“上古法器” | **存疑**：背景/类别描述，不是名称字段 | 20 |
| `server/assets/items/materials.toml:252` | `broken_artifact_scroll` | 残卷 | 古；“古卷残片” | **存疑**：普通历史描述，不是名称字段 | 25 |
| `server/assets/items/minerals.toml:92` | `yu_sui` | 玉髓 | 古；“古玉” | **存疑**：矿物类比描述，是否构成命名违规待定 | 14 |
| `server/assets/items/minerals.toml:104` | `wu_yao` | 乌曜石 | 玄；“玄石棺” | **存疑**：关联棺材形制的旧称，需用户决定是否联动 | 11 |
| `server/assets/items/pills.toml:471` | `po_jing_dan` | 破境丹 | 玄；“玄绒苔” | **存疑**：直接引用 `xuan_rong_tai` 的旧名，随植物裁决联动 | 12 |
| `server/assets/items/sword_materials.toml:83` | `sword_embryo_shard` | 剑胚残片 | 古；“古遗迹”“上古灵剑” | **存疑**：背景叙事，不是名称字段 | 20 |
| `server/assets/items/sword_materials.toml:158` | `scroll_sword_resonance` | 剑道残卷·共鸣要义 | 古；“古籍残页” | **存疑**：普通历史描述，不是名称字段 | 1 |
| `server/assets/items/weapons.toml:34` | `bronze_saber` | 青铜刀 | 古；“古旧铜料” | **存疑**：普通形容词“古旧”，疑似命名扫描误报 | 20 |

## 豁免核对

本次 **0 条豁免**。命中记录中没有“丹砂/朱砂/雄黄”一类已经进入世俗医药的矿名；“玄龟”“玄冰石”“古铜片”等即使未来被保留，也不能直接套用该例外，仍需用户单独裁决。

## 汇总与影响面

- **42 条唯一记录**：建议改 15 条、存疑 27 条、豁免 0 条；命中字段共 56 行。
- 全部记录按上述口径的引用数前三为：`body.guangbo_ticao` **62**、`anqi.echo_fractal` **34**、`broken_artifact_scroll` **25**。前三都主要是 description/历史兼容语境，并不代表应直接改名。
- 只看 15 条 `name` 命中时，影响面前三为：`hoe_xuantie` **24**、`xuan_iron` **22**、`gu_tong_pian` **20**。它们说明改名会同时触及 server 资产、配方/掉落、client 测试或 fixture，而不是改一行展示文字。
- 如果一条都不改，运行时不会因为这份清单立即改变；实际风险主要是存量名称继续触发新 PR 的命名 review，每次都要重复解释“这是继承的，不是本 PR 引入的”。一旦用户决定迁移，id 又不能直接替换，必须另开数据迁移与兼容方案。

本文件只记录证据与建议；没有修改任何 `id`、`name`、`description`、`docs/worldview.md` 或 `CLAUDE.md`。
