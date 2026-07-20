package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P0 —— cast_ticks ↔ 动画时长对拍测试（精度标准 #2
 * 三套断言的机械化）+ 现状不达标 allowlist 棘轮 + spec manifest 断言框架。
 *
 * <p>快照 {@code bong/technique_cast_ticks_snapshot.json} 由 server
 * {@code TECHNIQUE_DEFINITIONS} + yidao 5 招 spec（P4 增补，
 * {@code yidao_skill_spec().cast_ticks_base}）单向生成（重生成唯一入口
 * {@code cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot}），
 * 本测试只消费不维护——错误时长必须改动画对齐定义表，禁止「同步改快照混过关」
 * （plan §8.1 #4）。
 *
 * <p>三套断言（与精度标准 #2 严格同一时序模型）：
 * <ol>
 *   <li>普通非循环招（2 &lt; cast &lt; 40）：{@code endTick ∈ [cast+4, cast+8]}（recovery 红线）且非循环；</li>
 *   <li>瞬发招（cast ≤ 2）：总长 ∈ [6, 12] 且非循环（爆发帧 + 收势，不因 cast 短而砍收势）；</li>
 *   <li>长引导招（cast ≥ 40）：蓄力段动画 {@code isLoop} 且每个用到的轴在 endTick 有关键帧
 *       （防库坑 #1 单帧衰减的机械底线）；release 段存在性归 spec manifest（新交付时强制）。</li>
 * </ol>
 *
 * <p>现状不达标项进 {@link #CAST_ALIGNMENT_ALLOWLIST}（只缩不涨棘轮，冻结基线
 * {@link #P0_BASELINE} 永不追加）；<b>allowlist 清零 = P1-P4 完成的机械判据</b>
 * ——口径按 plan §8.1 #5 读作「allowlist 中<b>属于 P1-P4 重制清单</b>的条目清零」，
 * 明示归属外部 plan 的条目不计入但必须写明归属。
 *
 * <p>P6 收口（2026-07-21）后本表**余 1 条**：{@code woliu.vortex_resonance}
 * （归未消费骨架 plan-bughunt-woliu-resonance-loop-arm-decay-v1）。
 * {@code sword_path.heaven_gate} 已按 conventions §14.1 裁决移出，转由
 * {@link #FIXED_PHASE_CHARGE_SKILLS} 正向机械锁定。
 *
 * <p>两段式招的相位承接契约（conventions §14.2 裁决方案①）由
 * {@link #twoStageHandoffHoldsAcrossEveryReachableLoopPhase()} 覆盖全相位域锁定。
 */
class AnimCastTicksAlignmentTest {
    private static final String SNAPSHOT_RESOURCE = "bong/technique_cast_ticks_snapshot.json";
    private static final String REGEN_HINT =
        "快照由 server TECHNIQUE_DEFINITIONS 单向生成、禁止手改；重生成："
            + "cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot";

    /**
     * skill_id → 发射的动画文件名（plan 附录 A 审计矩阵同源；借用招映射到同一动画）。
     * value = null 表示该招当前无任何动画发射（进 {@link #MISSING_ANIM_ALLOWLIST}）。
     * npc.* 三招不在本表——NPC mob 无 PlayAnim 通道（plan §8.1 #2），动画维度 N/A。
     */
    private static final Map<String, String> SKILL_ANIM = buildSkillAnimMap();

    private static Map<String, String> buildSkillAnimMap() {
        Map<String, String> m = new TreeMap<>();
        m.put("sword.cleave", "sword_cleave");
        m.put("sword.thrust", "sword_thrust");
        m.put("sword.parry", "sword_parry");
        // P2 批次二后半（2026-07-19）：sword.infuse 两段式——蓄力段沿用
        // sword_infuse 文件名（重制为 isLoop 28t 横剑抚刃），release 段
        // sword_infuse_release 由 server 完成分支接力（两段式招映射蓄力段）。
        m.put("sword.infuse", "sword_infuse");
        m.put("movement.dash", "dash_forward");
        m.put("shield_block", "shield_raise");
        m.put("burst_meridian.beng_quan", "beng_quan");
        // P3 批次三（2026-07-19）：tie_shan_kao / xue_beng_bu 借用解除换专属
        // （原借 beng_quan，burst_meridian.rs 常量改指）；ni_mai_hu_ti 缺失补齐
        // （原 anim_id: None，出 MISSING_ANIM_ALLOWLIST）。
        m.put("burst_meridian.tie_shan_kao", "tie_shan_kao");
        m.put("burst_meridian.xue_beng_bu", "xue_beng_bu");
        m.put("burst_meridian.ni_mai_hu_ti", "ni_mai_hu_ti");
        // P3：全力一击两段借用解除（原借 windup_charge/release_burst）——charge
        // 为持续维持型循环（ChargingState 按住蓄力 + 释放/打断双路 StopAnim，
        // full_power_emit.rs），入 SUSTAINED_LOOP_EXCEPTIONS；release 瞬发。
        m.put("baomai.full_power_charge", "baomai_full_power_charge");
        m.put("baomai.full_power_release", "baomai_full_power_release");
        m.put("zhenmai.parry", "zhenmai_parry");
        m.put("zhenmai.neutralize", "zhenmai_neutralize");
        m.put("zhenmai.multipoint", "zhenmai_multipoint");
        m.put("zhenmai.harden", "zhenmai_harden");
        m.put("zhenmai.sever_chain", "zhenmai_sever_chain");
        // P3：woliu.vortex 缺口修正——调研证实并非「零发射」而是 field 出现时
        // lifecycle 借播 v2 站桩 vortex_spiral_stance（vfx_animation_trigger.rs
        // `emit_woliu_v1_vortex_visual_triggers`）；P3 换专属开涡 woliu_vortex_cast
        // 并出 MISSING_ANIM_ALLOWLIST。burst/mouth/pull/heart 借用/共用解除
        // （visual_for 改指，原借 palm_strike / palm_thrust / 共用 vacuum_lock /
        // 共用 vortex_spiral_stance）。
        m.put("woliu.vortex", "woliu_vortex_cast");
        m.put("woliu.hold", "vortex_palm_open");
        m.put("woliu.burst", "woliu_burst");
        m.put("woliu.mouth", "woliu_mouth");
        m.put("woliu.pull", "woliu_pull");
        m.put("woliu.heart", "woliu_heart");
        m.put("woliu.vacuum_palm", "woliu_vacuum_palm");
        m.put("woliu.vortex_shield", "woliu_vortex_shield");
        m.put("woliu.vacuum_lock", "woliu_vacuum_lock");
        m.put("woliu.vortex_resonance", "woliu_vortex_resonance");
        m.put("woliu.turbulence_burst", "woliu_turbulence_burst");
        m.put("dugu.shoot_needle", "dugu_needle_throw");
        // P3 去共用：淬毒专属（原与凝针共用一条 throw，仅去重 id 区分）。
        m.put("dugu.infuse_poison", "dugu_infuse_poison");
        m.put("tuike.don", "tuike_don_skin");
        m.put("tuike.shed", "tuike_shed_burst");
        m.put("tuike.transfer_taint", "tuike_taint_transfer");
        // P2 批次二后半（2026-07-19）：charge_carrier 两段式（真实 400t 通道，
        // 映射蓄力段 anqi_charge_carrier_loop；release 段 anqi_charge_carrier_release
        // 由 server CarrierChargeEndedEvent{full} 接力）。
        m.put("anqi.charge_carrier", "anqi_charge_carrier_loop");
        // P2 批次二前半（2026-07-19）：anqi 3 招 + sword_path 3 招去复用换专属动画。
        m.put("anqi.single_snipe", "anqi_single_snipe");
        m.put("anqi.multi_shot", "anqi_multi_shot");
        m.put("anqi.soul_inject", "anqi_soul_inject");
        // P2 批次二后半：armor_pierce / echo_fractal 专属单段（瞬发结算型长 cast，
        // 决策 (b)——resolver 立即结算无引导窗，cast_ticks 是元数据；仍驻
        // CAST_ALIGNMENT_ALLOWLIST，见该表注释）。
        m.put("anqi.armor_pierce", "anqi_armor_pierce");
        m.put("anqi.echo_fractal", "anqi_echo_fractal");
        m.put("body.guangbo_ticao", "guangbo_ticao");
        m.put("sword_path.condense_edge", "sword_path_condense_edge");
        m.put("sword_path.qi_slash", "sword_path_qi_slash");
        m.put("sword_path.resonance", "sword_path_resonance");
        m.put("sword_path.manifest", "sword_manifest_cast");
        m.put("sword_path.heaven_gate", "sword_heaven_gate_charge");
        m.put("morph.yixing", "morph_cast");
        // P4（2026-07-19）：yidao 5 招补齐（plan-yidao-v1 §5 欠账，矩阵外增补）。
        // 5 招全走 resolve_yidao_skill → insert_casting 真实长引导窗
        // （cast_ticks_base 100-1200t ≥ 40 长引导域，运行时经 yidao_cast_ticks
        // 按 mastery/平和色缩放——isLoop 蓄力段覆盖任意窗长），两段式映射蓄力段；
        // release 段（yidao_*_release）由 server complete_yidao_casts 有效结算
        // 分支接力，结构由各自三段式 spec manifest 锁定。快照来源同步扩展：
        // TECHNIQUE_DEFINITIONS + yidao_skill_spec 单向合并生成（见 server
        // technique_cast_ticks_snapshot_test.rs P4 注记）。
        m.put("yidao.meridian_repair", "yidao_meridian_repair_loop");
        m.put("yidao.contam_purge", "yidao_contam_purge_loop");
        m.put("yidao.emergency_resuscitate", "yidao_emergency_resuscitate_loop");
        m.put("yidao.life_extension", "yidao_life_extension_loop");
        m.put("yidao.mass_meridian_repair", "yidao_mass_meridian_repair_loop");
        return m;
    }

    /** NPC mob 施放、无 PlayAnim 通道的招（快照有、映射无的唯一合法集合）。 */
    private static final Set<String> NPC_SKILLS =
        Set.of("npc.heal_basic", "npc.buff_speed", "npc.buff_defense");

    /**
     * 持续维持型例外：循环 + StopAnim 停止路径是**正当设计**（按住/维持期间循环），
     * 时长对拍三套断言不适用；专属用例断言其循环结构不退化。
     *
     * <p>P3 批次三增录（每条入册前已核验停止路径，§13 #6 红线）：
     * <ul>
     *   <li>{@code baomai.full_power_charge}：ChargingState 按住蓄力状态机，释放
     *       （FullPowerReleasedEvent）与打断（ChargeInterruptedEvent）双退出路径
     *       均 StopAnim（full_power_emit.rs `emit_full_power_charging_clear_payloads`）；</li>
     *   <li>{@code woliu.vortex_shield}：VortexV2State 5s 持续窗，唯一退出路径 =
     *       active_until_tick 自然到期，由 `emit_woliu_v2_visual_stop_triggers`
     *       发 StopAnim（无提前破盾/主动取消机制，vfx_animation_trigger.rs:554）。</li>
     * </ul>
     */
    private static final Set<String> SUSTAINED_LOOP_EXCEPTIONS =
        Set.of("shield_block", "baomai.full_power_charge", "woliu.vortex_shield");

    /**
     * 长演出例外：一次性完整长动画（cast 只是入口时长，动画刻意超长演出），
     * 时长对拍不适用；专属用例断言其非循环长度不倒退。
     */
    private static final Set<String> LONG_FORM_EXCEPTIONS = Set.of("body.guangbo_ticao");

    /**
     * 现状时长对拍不达标 allowlist（P0 落档，2026-07-18 离线全量核算产出）——
     * **只许缩小不许增长**：修好一招必须删对应条目；清零 = P1-P4 完成的机械判据。
     */
    // P1 批次一（2026-07-19）删 10 条：sword.cleave/thrust + burst_meridian 三招
    // （beng_quan 重制 endTick=14 = 三借用方 cast 区间交集，tie_shan_kao/xue_beng_bu
    // 随之达标，棘轮强制同批删除）+ zhenmai 5 招。
    // P2 批次二前半（2026-07-19）删 5 条：anqi single_snipe/multi_shot/soul_inject
    // + sword_path qi_slash/resonance（6 招去复用专属化；condense_edge 原借 cleave
    // 时已达标故不在本表，专属化后 endTick=18 ∈ [16,20] 继续达标）。
    // P2 批次二后半（2026-07-19）删 1 条：sword.infuse（两段式落地，蓄力段
    // isLoop 28t 全轴同值闭环达标；charge_carrier 原本就不在表——windup_charge
    // 借用即为 loop 形态，换专属 loop 后继续达标）。
    // P3 批次三（2026-07-19）删 9 条：movement.dash（8t 瞬发重制）+
    // baomai.full_power_charge（专属循环，入 SUSTAINED_LOOP_EXCEPTIONS）+
    // baomai.full_power_release（专属 12t 瞬发）+ woliu.heart（专属 16t）+
    // woliu.vacuum_palm（12t 重制）+ woliu.vortex_shield（闭环 loop，入
    // SUSTAINED_LOOP_EXCEPTIONS）+ woliu.vacuum_lock（13t 重制）+
    // woliu.turbulence_burst（cast=40 核验为 resolver 立即结算无引导窗，入
    // INSTANT_RESOLVER_SKILLS）+ morph.yixing（同前，cast=60 无窗，入
    // INSTANT_RESOLVER_SKILLS）。
    // P6 收口（2026-07-21）删 1 条：sword_path.heaven_gate——口径之争按
    // conventions §14.1 裁决为**改判据不改动画**，移出本表进
    // {@link #FIXED_PHASE_CHARGE_SKILLS} 定长相位充能型分类契约（正向机械锁
    // 取代豁免）。裁决理由见该表注释。
    private static final Set<String> CAST_ALIGNMENT_ALLOWLIST = Set.of(
        // vortex_resonance 时长模型达标（80t loop 对齐 cast=80），但 11 个轴
        // endTick 无补帧/回绕跳变（库坑 #1 单帧衰减：双臂 10 轴末帧停在 t40 无
        // t80 补帧，外加 torso.pitch t80=0.0 ≠ 回绕锚点 t40=-0.06）——**存量 bug
        // 已登记归 plan-bughunt-woliu-resonance-loop-arm-decay-v1**（截至
        // 2026-07-21 仍是 docs/plans-skeleton/ 下**未消费骨架**，P0 ⬜、无开放
        // PR；本 plan §P3 明确全程零触碰以防重复修改，标准只防再犯），该 bugfix
        // merge 后删本条目。**本 plan 收官时唯一余项**（§8.1 #5 口径：明示归属
        // 外部 plan 的条目不计入 P1-P4 完成判据）。
        "woliu.vortex_resonance");

    /**
     * 瞬发结算型 resolver 招（review r2 分类契约，plan 附录 A）：gameplay 在
     * cast 起始 tick 立即结算（`resolve_anqi_skill` / `cast_manifest` 无
     * `Casting`、无 timer、无打断窗），cast_ticks 是元数据非施法窗。跨端时序
     * 契约 = **动画 strike 顶点与结算同帧（tick 0）**——开帧即命中姿态，其后
     * 只承担余韵与收势；由 instant spec manifest（strike_peak_tick=0）+
     * {@link #instantResolverSkillsPinStrikePeakAtTickZero()} 机械锁定。该类招
     * 不走 cast_ticks 时长对拍（元数据错配无意义），也不驻 allowlist（分类
     * 契约取代豁免）。新招入类前必须核验 resolver 确为立即结算。
     */
    private static final Map<String, String> INSTANT_RESOLVER_SKILLS = Map.of(
        "anqi.armor_pierce", "anqi_armor_pierce",
        "anqi.echo_fractal", "anqi_echo_fractal",
        "sword_path.manifest", "sword_manifest_cast",
        // P3 批次三增录（入类前均已核验 resolver 确为立即结算）：
        // turbulence_burst：`resolve_woliu_v2_skill` 同步一次性结算（woliu_v2/
        // skills.rs，零 Casting 组件，cast=40 仅作 Started.anim_duration_ticks
        // 透传）；yixing：`cast_morph_yixing` 双分支立即变形/解除（body_plan/
        // morph.rs，YIXING_CAST_TICKS=60 纯元数据）——两招均无引导窗可挂循环段。
        "woliu.turbulence_burst", "woliu_turbulence_burst",
        "morph.yixing", "morph_cast");

    /**
     * 两段式招登记表（P6 相位承接契约 §14.2 的机械锚点）：
     * skill_id → {蓄力/充能段动画, release 段动画}。
     *
     * <p>四招同构（plan P6「相位承接契约」范围）：前三条是**变长引导窗 + isLoop
     * 蓄力段**（引导时长随 mastery/平和色浮动，结束相位任意）；heaven_gate 是
     * **定长相位充能段**（见 {@link #FIXED_PHASE_CHARGE_SKILLS}），无相位歧义，
     * 在相位覆盖用例里作为确定性退化档单独断言（要求接缝**逐轴精确相等**，比
     * 变长档的预算断言更严）。yidao 5 招各自成对，合计 8 对。
     */
    private static final Map<String, String[]> TWO_STAGE_PAIRS = buildTwoStagePairs();

    private static Map<String, String[]> buildTwoStagePairs() {
        Map<String, String[]> m = new TreeMap<>();
        m.put("sword.infuse", new String[] {"sword_infuse", "sword_infuse_release"});
        m.put("anqi.charge_carrier",
            new String[] {"anqi_charge_carrier_loop", "anqi_charge_carrier_release"});
        m.put("sword_path.heaven_gate",
            new String[] {"sword_heaven_gate_charge", "sword_heaven_gate_release"});
        m.put("yidao.meridian_repair",
            new String[] {"yidao_meridian_repair_loop", "yidao_meridian_repair_release"});
        m.put("yidao.contam_purge",
            new String[] {"yidao_contam_purge_loop", "yidao_contam_purge_release"});
        m.put("yidao.emergency_resuscitate",
            new String[] {"yidao_emergency_resuscitate_loop",
                "yidao_emergency_resuscitate_release"});
        m.put("yidao.life_extension",
            new String[] {"yidao_life_extension_loop", "yidao_life_extension_release"});
        m.put("yidao.mass_meridian_repair",
            new String[] {"yidao_mass_meridian_repair_loop",
                "yidao_mass_meridian_repair_release"});
        return m;
    }

    /**
     * 相位承接姿态预算（弧度）：变长引导窗两段式招在**任意**结束相位上，蓄力段
     * 姿态与 release 段首帧姿态的逐轴最大差不得超过本值。
     *
     * <p>取值依据（conventions §14.2）：① 现网实测跨 8 对、全相位最大差 = 46°
     * （{@code yidao.contam_purge} 中段 {@code rightArm.bend}），本预算 60° 留
     * ~30% 余量而非空断言；② 交接由**外层淡出**承担真实姿态混合（见 §14.2 机制
     * 推导），混合起点就是蓄力段的当前相位姿态，不经过 vanilla；③ 本文档 §2.7
     * 已确立的先例是「从 vanilla 冷起手 45° 差用 3 tick fade-in 可接受」，而热
     * 交接严格温和于冷起手，故 60° 是保守上界。
     */
    private static final double PHASE_HANDOFF_BUDGET_RAD = Math.toRadians(60.0);

    /**
     * 定长相位充能型分类契约（P6 裁决，conventions §14.1）：充能段由**服务端确定
     * 性相位常量**驱动（非 mastery 缩放的变长引导窗），故不适用「长引导招蓄力段
     * 必须 isLoop」的判据——该判据的前提是窗长可变、需要循环覆盖任意时长。
     *
     * <p>裁决为**改判据不改动画**，理由（读代码实证，非口径偏好）：
     * <ul>
     *   <li>{@code sword_path.heaven_gate} 的充能相位是 {@code HEAVEN_GATE_CHARGE_END
     *       = 60}（server/src/sword_path/heaven_gate.rs:15）的**定长**相位，
     *       cast_ticks=80 是含后续判定相位的总窗，不是充能段时长；</li>
     *   <li>充能段资产是一条**单调递进的抬剑坡道**（rightArm.pitch 由 -0.698
     *       递进到 -2.688 rad），改成 isLoop 会在接缝制造这条坡道本不存在的
     *       回绕跳变，即为了满足判据而**引入**库坑 #1 类缺陷；</li>
     *   <li>定长相位下不存在「结束在任意相位」的歧义，接缝是确定的单点，可以
     *       比循环档**更严格**地锁死（逐轴精确相等，见
     *       {@link #fixedPhaseChargeSeamIsExactAndNonLooping()}）。</li>
     * </ul>
     *
     * <p>入类门槛（新招援引本类前逐条核验）：① 充能段时长 = 服务端具名相位常量
     * ② 充能段非循环且 endTick == 该常量 ③ 充能段末帧与 release 段首帧逐轴精确
     * 相等 ④ 两段 id 均被服务端映射表发射。value = 期望 endTick（= 服务端相位
     * 常量的客户端镜像，任一端漂移即判红）。
     */
    private static final Map<String, Integer> FIXED_PHASE_CHARGE_SKILLS = Map.of(
        "sword_path.heaven_gate", 60);

    /**
     * 无任何动画发射的招（D 级缺失，重制批次补动画后删条目）。
     * P3 批次三清零：ni_mai_hu_ti 补专属护体结印（burst_meridian.rs anim_id
     * None→Some）；woliu.vortex 核验发现并非零发射而是 lifecycle 借播 v2 站桩，
     * 换专属 woliu_vortex_cast 后走时长对拍主契约。
     */
    private static final Set<String> MISSING_ANIM_ALLOWLIST = Set.of();

    /**
     * P0 冻结基线（机器可执行的"只缩不涨"棘轮）：两份 allowlist 必须是本集合的
     * 子集——删除条目（动画达标）自然通过，新增任何条目立刻判红。本基线**永不
     * 追加**：新招不达标的唯一出路是按精度标准做动画，不得经由扩大 allowlist 放行。
     */
    private static final Set<String> P0_BASELINE = Set.of(
        // P0 时刻（2026-07-18）字面量冻结——**不得由 allowlist 动态计算**（自引用
        // 会让「只缩不涨」断言恒真失效，CodeRabbit r1 修正），也永不追加。
        "sword.cleave", "sword.thrust", "sword.infuse", "movement.dash",
        "burst_meridian.beng_quan", "burst_meridian.tie_shan_kao", "burst_meridian.xue_beng_bu",
        "burst_meridian.ni_mai_hu_ti",
        "baomai.full_power_charge", "baomai.full_power_release",
        "zhenmai.parry", "zhenmai.neutralize", "zhenmai.multipoint", "zhenmai.harden",
        "zhenmai.sever_chain",
        "woliu.vortex", "woliu.heart", "woliu.vacuum_palm", "woliu.vortex_shield",
        "woliu.vacuum_lock", "woliu.vortex_resonance", "woliu.turbulence_burst",
        "anqi.single_snipe", "anqi.multi_shot", "anqi.soul_inject", "anqi.armor_pierce",
        "anqi.echo_fractal",
        "sword_path.qi_slash", "sword_path.resonance", "sword_path.manifest",
        "sword_path.heaven_gate", "morph.yixing");

    // ---- 快照 / 动画元数据读取 ----

    private static Map<String, Integer> loadSnapshot() throws IOException {
        try (InputStream input = AnimCastTicksAlignmentTest.class.getClassLoader()
                .getResourceAsStream(SNAPSHOT_RESOURCE)) {
            assertNotNull(input, "缺少 cast_ticks 快照 fixture: " + SNAPSHOT_RESOURCE
                + " —— " + REGEN_HINT);
            JsonObject root = JsonParser
                .parseReader(new InputStreamReader(input, StandardCharsets.UTF_8))
                .getAsJsonObject();
            Map<String, Integer> snapshot = new TreeMap<>();
            for (String key : root.keySet()) {
                snapshot.put(key, root.get(key).getAsInt());
            }
            assertFalse(snapshot.isEmpty(), "cast_ticks 快照不应为空 —— " + REGEN_HINT);
            return snapshot;
        }
    }

    /** 生产动画资产根（与 {@code AnimWiringManifestTest#animationAssetRoot()} 同语义）。 */
    private static Path animationAssetRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/main/resources/assets/bong/player_animation"),
                cwd.resolve("client/src/main/resources/assets/bong/player_animation"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException(
            "无法定位 client main resources 的 player_animation 根（user.dir=" + cwd
                + "）——对拍必须绑定 main source set");
    }

    private record AnimMeta(int endTick, boolean isLoop, int returnTick, JsonArray moves) {
    }

    private static AnimMeta readAnim(String animName) throws IOException {
        Path file = animationAssetRoot().resolve(animName + ".json");
        assertTrue(Files.isRegularFile(file),
            "映射表指向的动画资产不存在：" + file + "——映射表与磁盘漂移，先修映射再谈对拍");
        JsonObject emote = JsonParser.parseString(Files.readString(file))
            .getAsJsonObject().getAsJsonObject("emote");
        return new AnimMeta(
            emote.get("endTick").getAsInt(),
            emote.has("isLoop") && emote.get("isLoop").getAsBoolean(),
            emote.has("returnTick") ? emote.get("returnTick").getAsInt() : 0,
            emote.getAsJsonArray("moves"));
    }

    /** 动画用到的全部轴（part.axis）集合，及 endTick 帧上出现的轴集合。 */
    private static void collectAxes(AnimMeta meta, Set<String> allAxes, Set<String> endTickAxes) {
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axes = move.get(part);
                if (!axes.isJsonObject()) {
                    continue;
                }
                for (String axis : axes.getAsJsonObject().keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    String key = part + "." + axis;
                    allAxes.add(key);
                    if (tick == meta.endTick()) {
                        endTickAxes.add(key);
                    }
                }
            }
        }
    }

    /**
     * 轴 → (tick → 值) 全量解析（循环补帧**同值**断言用——只查轴存在挡不住
     * endTick 写默认值/错值造成的循环接缝，CodeRabbit r1 修正）。
     */
    private static Map<String, Map<Integer, Double>> collectAxisValues(AnimMeta meta) {
        Map<String, Map<Integer, Double>> values = new HashMap<>();
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axes = move.get(part);
                if (!axes.isJsonObject()) {
                    continue;
                }
                for (String axis : axes.getAsJsonObject().keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    values.computeIfAbsent(part + "." + axis, k -> new HashMap<>())
                        .put(tick, axes.getAsJsonObject().get(axis).getAsDouble());
                }
            }
        }
        return values;
    }

    /**
     * 循环动画补帧同值检查：每个用到的轴在 endTick 必须有帧，且值 == 循环回绕锚点帧值
     * ——库坑 #1 的完整语义（同值 keyframe，非仅存在）。回绕锚点是 emote 的
     * {@code returnTick}（emotecraft 循环从 endTick 跳回 returnTick，而非一律回 tick 0；
     * 例：shield_raise 举盾 0→6 + hold 循环 6→18，returnTick=6，缝合要求 t18≡t6）。
     * 该轴在 returnTick 无关键帧时回落到该轴自身最小 tick 帧（returnTick=0 的存量
     * 资产行为不变）。返回违规轴描述列表（空=通过）。
     */
    private static List<String> loopSeamViolations(AnimMeta meta) {
        List<String> violations = new ArrayList<>();
        for (Map.Entry<String, Map<Integer, Double>> axisEntry : collectAxisValues(meta).entrySet()) {
            Map<Integer, Double> byTick = axisEntry.getValue();
            int firstTick = byTick.keySet().stream().min(Integer::compare).orElseThrow();
            int anchorTick = byTick.containsKey(meta.returnTick()) ? meta.returnTick() : firstTick;
            Double endValue = byTick.get(meta.endTick());
            if (endValue == null) {
                violations.add(axisEntry.getKey() + "（endTick 无关键帧）");
            } else if (!endValue.equals(byTick.get(anchorTick))) {
                violations.add(axisEntry.getKey() + "（endTick=" + endValue + " ≠ 回绕锚点帧(t"
                    + anchorTick + ")=" + byTick.get(anchorTick) + "，循环回绕跳变）");
            }
        }
        return violations;
    }

    /**
     * 在给定 tick 采样整套姿态（轴 → 值），相邻关键帧之间线性插值、两端钳制。
     *
     * <p>用线性插值而非复刻 easing 曲线是**刻意**的：本采样只用于衡量两段动画
     * 之间的姿态差幅度，easing 只改变同一对端点之间的走法、不改变端点值，故对
     * 「最大差」这个量的判定不产生影响；复刻 Ease 家族反而会把测试绑死在库的
     * 内部实现上（测契约不测实现）。
     */
    private static Map<String, Double> poseAt(AnimMeta meta, int tick) {
        Map<String, Double> pose = new TreeMap<>();
        for (Map.Entry<String, Map<Integer, Double>> axisEntry
                : collectAxisValues(meta).entrySet()) {
            Map<Integer, Double> byTick = axisEntry.getValue();
            List<Integer> ticks = new ArrayList<>(byTick.keySet());
            Collections.sort(ticks);
            int first = ticks.get(0);
            int last = ticks.get(ticks.size() - 1);
            double value;
            if (tick <= first) {
                value = byTick.get(first);
            } else if (tick >= last) {
                value = byTick.get(last);
            } else {
                int lo = first;
                int hi = last;
                for (int candidate : ticks) {
                    if (candidate <= tick) {
                        lo = candidate;
                    }
                    if (candidate >= tick) {
                        hi = candidate;
                        break;
                    }
                }
                double lowValue = byTick.get(lo);
                double highValue = byTick.get(hi);
                value = hi == lo
                    ? highValue
                    : lowValue + (highValue - lowValue) * ((double) (tick - lo) / (hi - lo));
            }
            pose.put(axisEntry.getKey(), value);
        }
        return pose;
    }

    /**
     * 两套姿态的逐轴最大差。只比较**两侧都声明**的轴；仅单侧声明的轴另行校验
     * 「其值必须为中立 0.0」——否则一段动画悄悄少写一条非中立轴就能绕过差值断言
     * （{@code body.x} 这类两侧写法不一致但恒为 0.0 的存量差异因此被正确放行）。
     * 返回 {@code [最大差, 描述]}，描述在超预算时进断言消息。
     */
    private static Map.Entry<Double, String> maxAxisDelta(
        Map<String, Double> left,
        Map<String, Double> right,
        String leftLabel,
        String rightLabel
    ) {
        double worst = 0.0;
        String worstAxis = "（无共同轴）";
        for (Map.Entry<String, Double> entry : left.entrySet()) {
            Double other = right.get(entry.getKey());
            if (other == null) {
                assertEquals(0.0, entry.getValue(), 1e-9,
                    "轴 `" + entry.getKey() + "` 只在 " + leftLabel + " 声明而 " + rightLabel
                        + " 未声明，且其值非中立 0.0——单侧非中立轴会在交接瞬间凭空跳变，"
                        + "两段必须同时声明该轴");
                continue;
            }
            double delta = Math.abs(entry.getValue() - other);
            if (delta > worst) {
                worst = delta;
                worstAxis = entry.getKey();
            }
        }
        for (Map.Entry<String, Double> entry : right.entrySet()) {
            if (!left.containsKey(entry.getKey())) {
                assertEquals(0.0, entry.getValue(), 1e-9,
                    "轴 `" + entry.getKey() + "` 只在 " + rightLabel + " 声明而 " + leftLabel
                        + " 未声明，且其值非中立 0.0——单侧非中立轴会在交接瞬间凭空跳变，"
                        + "两段必须同时声明该轴");
            }
        }
        return Map.entry(worst, worstAxis);
    }

    /** 精度标准 #2 三套断言；返回 null=达标，非 null=不达标原因（供 allowlist 双向棘轮复用）。 */
    private static String alignmentFailure(int cast, AnimMeta meta) {
        if (cast <= 2) {
            if (meta.isLoop()) {
                return "瞬发招动画不应循环（isLoop=true）";
            }
            if (meta.endTick() < 6 || meta.endTick() > 12) {
                return "瞬发招总长 " + meta.endTick() + " ∉ [6,12]（爆发帧+收势）";
            }
            return null;
        }
        if (cast >= 40) {
            if (!meta.isLoop()) {
                return "长引导招蓄力段应为循环（isLoop=false）——须拆循环蓄力段+release 段两段";
            }
            List<String> seams = loopSeamViolations(meta);
            if (!seams.isEmpty()) {
                return "循环蓄力段 " + seams.size() + " 个轴违反 endTick 同值补帧（库坑 #1）：" + seams;
            }
            return null;
        }
        if (meta.isLoop()) {
            return "普通非循环招不应循环（isLoop=true）";
        }
        if (meta.endTick() < cast + 4 || meta.endTick() > cast + 8) {
            return "endTick " + meta.endTick() + " ∉ [" + (cast + 4) + "," + (cast + 8)
                + "]（cast 完成=发力顶点 + recovery 4-8 tick）";
        }
        return null;
    }

    // ---- 用例 ----

    /** 快照覆盖：映射表每招在快照有 cast_ticks；快照多出的键只能是 npc 三招。 */
    @Test
    void snapshotCoversEveryMappedSkillAndOnlyNpcSkillsAreUnmapped() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        for (String skillId : SKILL_ANIM.keySet()) {
            assertTrue(snapshot.containsKey(skillId),
                "映射表招式 `" + skillId + "` 不在 cast_ticks 快照内——server 定义表删招"
                    + "未同步映射表，或快照过期 —— " + REGEN_HINT);
        }
        Set<String> unmapped = new HashSet<>(snapshot.keySet());
        unmapped.removeAll(SKILL_ANIM.keySet());
        assertEquals(NPC_SKILLS, unmapped,
            "快照有而映射表无的招必须恰为 npc 三招（NPC mob 无 PlayAnim 通道，"
                + "plan §8.1 #2）——出现其他条目说明 server 新增招未入对拍映射表");
    }

    /** 主契约：非 allowlist / 非例外的招全部满足精度标准 #2 三套断言。 */
    @Test
    void castAlignmentContractHoldsForEverySkillOutsideAllowlist() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        List<String> violations = new ArrayList<>();
        for (Map.Entry<String, String> entry : SKILL_ANIM.entrySet()) {
            String skillId = entry.getKey();
            if (SUSTAINED_LOOP_EXCEPTIONS.contains(skillId)
                || LONG_FORM_EXCEPTIONS.contains(skillId)
                || CAST_ALIGNMENT_ALLOWLIST.contains(skillId)
                || MISSING_ANIM_ALLOWLIST.contains(skillId)
                // 瞬发结算型分类契约：不走 cast_ticks 时长对拍，由
                // instantResolverSkillsPinStrikePeakAtTickZero + instant manifest 锁定。
                || INSTANT_RESOLVER_SKILLS.containsKey(skillId)
                // 定长相位充能型分类契约（P6 §14.1）：充能段由服务端确定性相位常量
                // 驱动，不适用「长引导必须 isLoop」判据，由
                // fixedPhaseChargeSeamIsExactAndNonLooping 以更严的零容差接缝锁定。
                || FIXED_PHASE_CHARGE_SKILLS.containsKey(skillId)) {
                continue;
            }
            assertNotNull(entry.getValue(),
                "招式 `" + skillId + "` 无动画映射却不在 MISSING_ANIM_ALLOWLIST——"
                    + "缺动画必须显式入 allowlist 留痕");
            String failure = alignmentFailure(snapshot.get(skillId), readAnim(entry.getValue()));
            if (failure != null) {
                violations.add(skillId + "（" + entry.getValue() + "）：" + failure);
            }
        }
        assertTrue(violations.isEmpty(),
            "以下招式违反精度标准 #2 时长对拍且不在 allowlist：" + violations
                + "——新招/重制动画必须直接达标，不得扩大 allowlist（冻结基线只缩不涨）");
    }

    /** 棘轮下界：allowlist 条目必须确实不达标——动画修好即强制删条目。 */
    @Test
    void allowlistEntriesActuallyFailAlignment() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        for (String skillId : CAST_ALIGNMENT_ALLOWLIST) {
            String anim = SKILL_ANIM.get(skillId);
            assertNotNull(anim,
                "allowlist 条目 `" + skillId + "` 无动画映射——应移入 MISSING_ANIM_ALLOWLIST");
            String failure = alignmentFailure(snapshot.get(skillId), readAnim(anim));
            assertNotNull(failure,
                "allowlist 条目 `" + skillId + "` 的动画 `" + anim + "` 已达标——"
                    + "必须立刻从 CAST_ALIGNMENT_ALLOWLIST 删除该条目（棘轮只缩不涨）");
        }
        for (String skillId : MISSING_ANIM_ALLOWLIST) {
            assertTrue(SKILL_ANIM.containsKey(skillId) && SKILL_ANIM.get(skillId) == null,
                "MISSING_ANIM_ALLOWLIST 条目 `" + skillId + "` 已有动画映射——"
                    + "必须删条目并让其走时长对拍主契约");
        }
    }

    /** 冻结基线棘轮：两份 allowlist ⊆ P0 基线（只缩不涨，永不追加）。 */
    @Test
    void allowlistsOnlyShrinkAgainstFrozenBaseline() {
        assertTrue(P0_BASELINE.containsAll(CAST_ALIGNMENT_ALLOWLIST),
            "CAST_ALIGNMENT_ALLOWLIST 出现 P0 冻结基线之外的条目："
                + CAST_ALIGNMENT_ALLOWLIST.stream()
                    .filter(id -> !P0_BASELINE.contains(id)).toList()
                + "——基线永不追加，新缺口唯一出路是按精度标准做动画");
        assertTrue(P0_BASELINE.containsAll(MISSING_ANIM_ALLOWLIST),
            "MISSING_ANIM_ALLOWLIST 出现 P0 冻结基线之外的条目");
        assertTrue(SKILL_ANIM.keySet().containsAll(P0_BASELINE),
            "基线含映射表之外的僵尸条目：" + P0_BASELINE.stream()
                .filter(id -> !SKILL_ANIM.containsKey(id)).toList());
    }

    /**
     * 瞬发结算型分类契约 pin（review r2）：结算 tick = strike 顶点 tick = 0。
     * 每招必须 ① 映射到声明的专属动画 ② 不驻 allowlist（分类契约取代豁免）
     * ③ 有 instant spec manifest 且 strike_peak_tick=0（manifest 结构与轴级
     * 断言归 {@link #specManifestsEnforcePrecisionStandardMechanically()} 的
     * instant 分支：strike 从 0 起、主打击轴 tick 0 落帧、密度/easing 同标准）。
     */
    @Test
    void instantResolverSkillsPinStrikePeakAtTickZero() throws IOException {
        for (Map.Entry<String, String> entry : INSTANT_RESOLVER_SKILLS.entrySet()) {
            String skillId = entry.getKey();
            assertEquals(entry.getValue(), SKILL_ANIM.get(skillId),
                skillId + " 的映射动画与瞬发分类声明不一致——改映射必须同步 INSTANT_RESOLVER_SKILLS");
            assertFalse(CAST_ALIGNMENT_ALLOWLIST.contains(skillId),
                skillId + " 同时出现在瞬发分类与 CAST_ALIGNMENT_ALLOWLIST——分类契约取代豁免，二者互斥");
            Path manifestFile = manifestRoot().resolve(entry.getValue() + ".json");
            assertTrue(Files.isRegularFile(manifestFile),
                skillId + " 缺 instant spec manifest：" + manifestFile
                    + "——瞬发结算型必须机械锁定 strike 顶点=tick 0");
            JsonObject manifest =
                JsonParser.parseString(Files.readString(manifestFile)).getAsJsonObject();
            assertTrue(manifest.has("instant") && manifest.get("instant").getAsBoolean(),
                skillId + " 的 spec manifest 未声明 instant=true——瞬发结算型必须走 instant 契约");
            assertEquals(0, manifest.get("strike_peak_tick").getAsInt(),
                skillId + " strike_peak_tick 必须为 0：resolver 在 cast 起始 tick 立即结算，"
                    + "视觉命中顶点必须与结算同帧");
        }
    }

    /**
     * 定长相位充能型分类契约 pin（P6 裁决，conventions §14.1）——**取代 heaven_gate
     * 原先的 allowlist 豁免**，把「跳过检查」换成比循环档更严的正向锁：
     * ① 充能段非循环 ② endTick == 服务端相位常量镜像 ③ 充能段**末帧**与 release
     * 段**首帧**逐轴精确相等（确定性接缝，零容差）④ 不驻 allowlist（互斥）。
     */
    @Test
    void fixedPhaseChargeSeamIsExactAndNonLooping() throws IOException {
        for (Map.Entry<String, Integer> entry : FIXED_PHASE_CHARGE_SKILLS.entrySet()) {
            String skillId = entry.getKey();
            int expectedEndTick = entry.getValue();
            assertFalse(CAST_ALIGNMENT_ALLOWLIST.contains(skillId),
                skillId + " 同时出现在定长相位充能分类与 CAST_ALIGNMENT_ALLOWLIST——"
                    + "分类契约取代豁免，二者互斥");
            String[] pair = TWO_STAGE_PAIRS.get(skillId);
            assertNotNull(pair,
                skillId + " 声明为定长相位充能型却未登记两段式配对——"
                    + "两段式招必须在 TWO_STAGE_PAIRS 有 {充能段, release 段}");
            assertEquals(pair[0], SKILL_ANIM.get(skillId),
                skillId + " 的映射动画与两段式配对声明不一致——改映射必须同步 TWO_STAGE_PAIRS");

            AnimMeta charge = readAnim(pair[0]);
            AnimMeta release = readAnim(pair[1]);
            assertFalse(charge.isLoop(),
                skillId + " 充能段 `" + pair[0] + "` 不应为循环：定长相位无需循环覆盖任意窗长，"
                    + "改 isLoop 会给这条单调递进坡道引入本不存在的回绕跳变（conventions §14.1）");
            assertEquals(expectedEndTick, charge.endTick(),
                skillId + " 充能段 endTick=" + charge.endTick() + " 与服务端相位常量镜像 "
                    + expectedEndTick + " 不一致——server/src/sword_path/heaven_gate.rs 的"
                    + " HEAVEN_GATE_CHARGE_END 与本资产任一端漂移都必须同步改，"
                    + "否则充能动画与充能相位错位");

            Map.Entry<Double, String> worst = maxAxisDelta(
                poseAt(charge, charge.endTick()), poseAt(release, 0),
                pair[0] + "@t" + charge.endTick(), pair[1] + "@t0");
            assertEquals(0.0, worst.getKey(), 1e-9,
                skillId + " 充能段末帧与 release 段首帧不一致（最大差轴 `" + worst.getValue()
                    + "` = " + worst.getKey() + " rad）——定长相位的接缝是确定性单点，"
                    + "必须逐轴精确相等，不吃相位混合预算");
        }
    }

    /**
     * 两段式「相位承接契约」相位覆盖用例（P6 裁决方案①，conventions §14.2）。
     *
     * <p>锁的是：变长引导窗结束在**任意相位**时，蓄力段当时的姿态与 release 段
     * 首帧姿态的差都在混合预算内——因为客户端交接是「外层淡出的蓄力段混入下层
     * 刚起播的 release 段」，混合起点是蓄力段的**当前相位姿态**（不是 tick 0、
     * 也不经过 vanilla），所以契约必须对整个相位域成立，而不只对基位成立。
     *
     * <p>覆盖口径：直接枚举 {@code [0, 周期)} 的**全部整数相位**——这是 plan 要求
     * 的「可达 cast_ticks 对 loop 周期取余」的严格超集（无论 cast_ticks 怎样随
     * mastery/平和色浮动，其余数必落在本域内），且对未来 cast_ticks 调参免疫。
     * plan 点名的三个相位（基位 / 中间相位 / 周期末相位）额外单独断言留痕。
     */
    @Test
    void twoStageHandoffHoldsAcrossEveryReachableLoopPhase() throws IOException {
        List<String> violations = new ArrayList<>();
        for (Map.Entry<String, String[]> entry : TWO_STAGE_PAIRS.entrySet()) {
            String skillId = entry.getKey();
            if (FIXED_PHASE_CHARGE_SKILLS.containsKey(skillId)) {
                // 定长相位充能段无相位歧义，接缝由 fixedPhaseChargeSeamIsExactAndNonLooping
                // 以零容差锁死（更严），不重复走预算断言。
                continue;
            }
            String loopName = entry.getValue()[0];
            String releaseName = entry.getValue()[1];
            AnimMeta loop = readAnim(loopName);
            AnimMeta release = readAnim(releaseName);
            assertTrue(loop.isLoop(),
                skillId + " 蓄力段 `" + loopName + "` 应为循环——变长引导窗必须由循环段覆盖任意窗长");
            assertFalse(release.isLoop(),
                skillId + " release 段 `" + releaseName + "` 不应循环——收势是一次性演出");

            Map<String, Double> releaseFirst = poseAt(release, 0);
            int period = loop.endTick();
            for (int phase = 0; phase < period; phase++) {
                Map.Entry<Double, String> worst = maxAxisDelta(
                    poseAt(loop, phase), releaseFirst,
                    loopName + "@相位" + phase, releaseName + "@t0");
                if (worst.getKey() > PHASE_HANDOFF_BUDGET_RAD) {
                    violations.add(skillId + " 相位 " + phase + "/" + period + " 轴 `"
                        + worst.getValue() + "` 差 "
                        + String.format("%.1f", Math.toDegrees(worst.getKey())) + "° > 预算 "
                        + String.format("%.1f", Math.toDegrees(PHASE_HANDOFF_BUDGET_RAD)) + "°");
                }
            }
            // plan 点名的三个相位单独留痕（基位 / 中间相位 / 周期末相位）。
            for (int phase : List.of(0, period / 2, period - 1)) {
                Map.Entry<Double, String> worst = maxAxisDelta(
                    poseAt(loop, phase), releaseFirst,
                    loopName + "@相位" + phase, releaseName + "@t0");
                assertTrue(worst.getKey() <= PHASE_HANDOFF_BUDGET_RAD,
                    skillId + " 在点名相位 " + phase + "（周期 " + period + "）承接超预算：轴 `"
                        + worst.getValue() + "` 差 "
                        + String.format("%.1f", Math.toDegrees(worst.getKey())) + "°，预算 "
                        + String.format("%.1f", Math.toDegrees(PHASE_HANDOFF_BUDGET_RAD))
                        + "°——须收窄 release 首帧与蓄力段的姿态距离（conventions §14.2）");
            }
        }
        assertTrue(violations.isEmpty(),
            "两段式相位承接超出混合预算：" + violations
                + "——release 首帧必须落在蓄力段整个相位域的混合可达范围内");
    }

    /** 两段式登记表自洽：每招都在映射表里、两段资产都存在且不同名。 */
    @Test
    void twoStagePairsAreRegisteredConsistently() throws IOException {
        for (Map.Entry<String, String[]> entry : TWO_STAGE_PAIRS.entrySet()) {
            String skillId = entry.getKey();
            String[] pair = entry.getValue();
            assertEquals(2, pair.length, skillId + " 两段式配对必须恰好 {蓄力段, release 段}");
            assertNotEquals(pair[0], pair[1],
                skillId + " 两段指向同一动画——两段式的意义就是两段可分辨的演出");
            assertEquals(pair[0], SKILL_ANIM.get(skillId),
                skillId + " 的 SKILL_ANIM 映射必须是蓄力段（对拍主契约按蓄力段判长引导）");
            // 两段资产都必须真实存在（readAnim 内部对缺文件硬失败）。
            readAnim(pair[0]);
            readAnim(pair[1]);
        }
    }

    /**
     * 持续维持型例外（shield_block）：循环结构 + 每轴 endTick 有帧不退化。
     * 注意此处刻意只查**帧存在**不查同值——shield_raise 是「举起后保持」语义，
     * 首尾帧值不同是既有资产的刻意设计（0→6 举起后循环维持末段姿态），与
     * 蓄力循环段的同值补帧红线（{@link #loopSeamViolations}）分属两种循环形态。
     */
    @Test
    void sustainedLoopExceptionKeepsLoopStructure() throws IOException {
        for (String skillId : SUSTAINED_LOOP_EXCEPTIONS) {
            AnimMeta meta = readAnim(SKILL_ANIM.get(skillId));
            assertTrue(meta.isLoop(),
                skillId + " 是持续维持型技能，动画应保持 isLoop=true（举盾按住持续 + "
                    + "StopAnim 停止路径）——改非循环需同步撤销本例外并过时长对拍");
            Set<String> all = new HashSet<>();
            Set<String> atEnd = new HashSet<>();
            collectAxes(meta, all, atEnd);
            assertEquals(all, atEnd,
                skillId + " 循环动画每个用到的轴必须在 endTick 有关键帧（库坑 #1 单帧衰减底线）");
        }
    }

    /** 长演出例外（guangbo_ticao）：非循环且长度不倒退（防退化成快闪）。 */
    @Test
    void longFormExceptionKeepsFullLengthNonLoop() throws IOException {
        for (String skillId : LONG_FORM_EXCEPTIONS) {
            AnimMeta meta = readAnim(SKILL_ANIM.get(skillId));
            assertFalse(meta.isLoop(), skillId + " 长演出动画应为非循环一次性播放");
            assertTrue(meta.endTick() >= 100,
                skillId + " 长演出动画 endTick=" + meta.endTick()
                    + " 低于 100——A 级基准资产不得退化（当前基准 150）");
        }
    }

    /**
     * spec manifest 断言框架（精度标准机械化，仅对本 plan 新交付动画强制）：
     * {@code bong/anim_spec_manifests/<anim>.json} 声明三段边界后逐项断言——
     * 三段各 ≥2 帧点、主轴相邻帧间隔 ≤4 tick、easing 显式且打击轴禁 linear、
     * leg.pitch ≤ 40°（0.698 rad）、循环每轴 endTick 补帧。目录随批次填充。
     */
    @Test
    void specManifestsEnforcePrecisionStandardMechanically() throws IOException {
        Path manifestDir = manifestRoot();
        assertTrue(Files.isDirectory(manifestDir),
            "spec manifest 目录缺失：" + manifestDir + "——P0 框架要求目录存在（随批次填充）");
        try (Stream<Path> files = Files.list(manifestDir)) {
            for (Path manifestFile : files
                    .filter(p -> p.getFileName().toString().endsWith(".json")).toList()) {
                assertManifestHolds(manifestFile);
            }
        }
    }

    private static Path manifestRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/test/resources/bong/anim_spec_manifests"),
                cwd.resolve("client/src/test/resources/bong/anim_spec_manifests"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException("无法定位 anim_spec_manifests 目录（user.dir=" + cwd + "）");
    }

    private static void assertManifestHolds(Path manifestFile) throws IOException {
        String animName = manifestFile.getFileName().toString().replace(".json", "");
        JsonObject manifest = JsonParser.parseString(Files.readString(manifestFile)).getAsJsonObject();
        AnimMeta meta = readAnim(animName);

        // 段式 manifest（P2 后半，review 返工新增）：两段式招的 loop 蓄力段 /
        // 定长充能段没有三段式结构（strike 归其 release 段），改锁段语义——
        // segment="loop"：isLoop=true + 每轴 endTick 同值补帧（库坑 #1）；
        // segment="charge_hold"：非循环定长充能段（endTick=充能窗长，末帧=交接帧）。
        // 两型共同：endTick pin、主轴密度 ≤4t、easing 显式非 linear、leg.pitch 红线。
        if (manifest.has("segment")) {
            String segment = manifest.get("segment").getAsString();
            assertTrue(segment.equals("loop") || segment.equals("charge_hold"),
                animName + " manifest segment 类型未知：`" + segment
                    + "`（合法：loop / charge_hold）");
            assertEquals(segment.equals("loop"), meta.isLoop(),
                animName + " segment=" + segment + " 与 isLoop=" + meta.isLoop()
                    + " 不符——loop 蓄力段必须 isLoop=true（配 StopAnim 停止路径），"
                    + "charge_hold 定长充能段必须非循环（段长=通道相位长，无死帧）");
            assertEquals(manifest.get("expected_end_tick").getAsInt(), meta.endTick(),
                animName + " endTick=" + meta.endTick() + " 与 manifest expected_end_tick 漂移"
                    + "——段长即通道契约（loop 周期 / 充能窗长），改动画必须同步 manifest");
            List<String> primaryAxes = new ArrayList<>();
            for (JsonElement axis : manifest.getAsJsonArray("primary_axes")) {
                primaryAxes.add(axis.getAsString());
            }
            assertFalse(primaryAxes.isEmpty(),
                animName + " segment manifest 必须声明至少一个主轴（primary_axes）");
            AxisWalk walk = walkAxes(animName, meta);
            for (String axis : primaryAxes) {
                assertAxisDense(animName, walk, axis);
            }
            if (meta.isLoop()) {
                List<String> seams = loopSeamViolations(meta);
                assertTrue(seams.isEmpty(),
                    animName + " 循环动画违反 endTick 同值补帧（库坑 #1，循环回绕跳变）：" + seams);
            }
            return;
        }

        // instant 型 manifest（review r2 瞬发结算型分类契约）：无 anticipation 段
        // ——gameplay 已在 cast 起始 tick 结算，开帧必须就是命中顶点。strike 从
        // 0 起、recovery 收在 endTick；每条主打击轴必须在 tick 0 落帧（顶点
        // 落帧），密度/easing/leg.pitch 与三段式同标准。
        if (manifest.has("instant")) {
            assertTrue(manifest.get("instant").getAsBoolean(),
                animName + " instant 字段只允许 true（非瞬发招不要写该字段）");
            assertFalse(meta.isLoop(), animName + " 瞬发结算型动画必须非循环");
            assertEquals(0, manifest.get("strike_peak_tick").getAsInt(),
                animName + " strike_peak_tick 必须为 0（结算与视觉命中同帧，"
                    + "review r2 跨端时序契约）");
            JsonArray strikeRange = manifest.getAsJsonArray("strike");
            int strikeFrom = strikeRange.get(0).getAsInt();
            int strikeTo = strikeRange.get(1).getAsInt();
            JsonArray recoveryRange = manifest.getAsJsonArray("recovery");
            assertEquals(0, strikeFrom,
                animName + " instant strike 必须从 tick 0 起（顶点即开帧，无 anticipation）");
            assertTrue(strikeFrom < strikeTo, animName + " strike 段必须 from < to");
            assertTrue(strikeTo <= recoveryRange.get(0).getAsInt(),
                animName + " strike 与 recovery 必须有序不重叠");
            assertEquals(meta.endTick(), recoveryRange.get(1).getAsInt(),
                animName + " recovery 必须收在 endTick=" + meta.endTick());
            List<String> instantAxes = new ArrayList<>();
            for (JsonElement axis : manifest.getAsJsonArray("strike_axes")) {
                instantAxes.add(axis.getAsString());
            }
            assertFalse(instantAxes.isEmpty(), animName + " instant manifest 必须声明主打击轴");
            AxisWalk instantWalk = walkAxes(animName, meta);
            for (String axis : instantAxes) {
                assertAxisDense(animName, instantWalk, axis);
                assertTrue(instantWalk.ticks().get(axis).contains(0),
                    animName + " 主打击轴 `" + axis + "` 必须在 tick 0 有关键帧（顶点落帧）");
            }
            return;
        }

        // 三段边界：anticipation/strike/recovery 的 [from, to]（tick，含端点）。
        Map<String, int[]> phases = new HashMap<>();
        for (String phase : List.of("anticipation", "strike", "recovery")) {
            assertTrue(manifest.has(phase),
                animName + " manifest 缺少 " + phase + " 段声明（三段式结构红线）");
            JsonArray range = manifest.getAsJsonArray(phase);
            phases.put(phase, new int[] {range.get(0).getAsInt(), range.get(1).getAsInt()});
        }
        // 三段区间结构校验：各段 from<to、有序不重叠（相邻可共享边界 tick=hold 衔接）、
        // 首段从 0 起、末段收在 endTick——防「三段都填 [0,endTick]」的空声明混过
        // （CodeRabbit r1 修正）。
        int[] ant = phases.get("anticipation");
        int[] strike = phases.get("strike");
        int[] recovery = phases.get("recovery");
        for (Map.Entry<String, int[]> phase : phases.entrySet()) {
            assertTrue(phase.getValue()[0] < phase.getValue()[1],
                animName + " " + phase.getKey() + " 段 [" + phase.getValue()[0] + ","
                    + phase.getValue()[1] + "] 必须 from < to");
        }
        assertEquals(0, ant[0], animName + " anticipation 必须从 tick 0 起");
        assertTrue(ant[1] <= strike[0],
            animName + " anticipation(" + ant[1] + ") 与 strike(" + strike[0]
                + ") 必须有序不重叠（≤，可共享边界 tick）");
        assertTrue(strike[1] <= recovery[0],
            animName + " strike(" + strike[1] + ") 与 recovery(" + recovery[0]
                + ") 必须有序不重叠");
        assertEquals(meta.endTick(), recovery[1],
            animName + " recovery 必须收在 endTick=" + meta.endTick() + "（收势到尾，"
                + "精度标准 #1/#2——声明短于动画会漏检尾段密度）");
        // 主打击轴清单（strike 段密度 + 禁 linear 的检查对象）。
        List<String> strikeAxes = new ArrayList<>();
        for (JsonElement axis : manifest.getAsJsonArray("strike_axes")) {
            strikeAxes.add(axis.getAsString());
        }
        assertFalse(strikeAxes.isEmpty(), animName + " manifest 必须声明至少一个主打击轴");

        AxisWalk walk = walkAxes(animName, meta);
        Map<String, List<Integer>> axisTicks = walk.ticks();
        // 三段各 ≥2 帧点（按任意轴在段内的帧点计）。
        Set<Integer> allTicks = new HashSet<>();
        axisTicks.values().forEach(allTicks::addAll);
        for (Map.Entry<String, int[]> phase : phases.entrySet()) {
            long inPhase = allTicks.stream()
                .filter(t -> t >= phase.getValue()[0] && t <= phase.getValue()[1]).count();
            assertTrue(inPhase >= 2,
                animName + " " + phase.getKey() + " 段仅 " + inPhase + " 个帧点（应 ≥2，三段式结构红线）");
        }
        // 主打击轴：相邻帧间隔 ≤4 tick + 禁 linear。
        for (String axis : strikeAxes) {
            assertAxisDense(animName, walk, axis);
        }
        // 循环动画每轴 endTick 同值补帧（库坑 #1 完整语义：值相同，非仅存在）。
        if (meta.isLoop()) {
            List<String> seams = loopSeamViolations(meta);
            assertTrue(seams.isEmpty(),
                animName + " 循环动画违反 endTick 同值补帧（库坑 #1，循环回绕跳变）：" + seams);
        }
    }

    /** 每轴关键帧走查结果：帧点清单 + easing 集合（key = {@code part.axis}）。 */
    private record AxisWalk(Map<String, List<Integer>> ticks, Map<String, Set<String>> easings) {}

    /** 走一遍全部关键帧：收集每轴帧点/easing，并断言 easing 显式 + leg.pitch 红线。 */
    private static AxisWalk walkAxes(String animName, AnimMeta meta) {
        Map<String, List<Integer>> axisTicks = new HashMap<>();
        Map<String, Set<String>> axisEasings = new HashMap<>();
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            String easing = move.has("easing") ? move.get("easing").getAsString() : null;
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axesElement = move.get(part);
                if (!axesElement.isJsonObject()) {
                    continue;
                }
                JsonObject axes = axesElement.getAsJsonObject();
                for (String axis : axes.keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    String key = part + "." + axis;
                    axisTicks.computeIfAbsent(key, k -> new ArrayList<>()).add(tick);
                    assertNotNull(easing,
                        animName + " tick " + tick + " 帧未显式声明 easing（精度标准 #3）");
                    axisEasings.computeIfAbsent(key, k -> new HashSet<>()).add(easing);
                    // leg.pitch ≤ 40°（弧度 0.698）——库坑 #2。
                    if ((part.contains("leg") || part.contains("Leg")) && axis.equals("pitch")) {
                        double value = Math.abs(axes.get(axis).getAsDouble());
                        assertTrue(value <= 0.699,
                            animName + " " + key + "@" + tick + " = " + value
                                + " rad 超过 40°（0.698 rad）——大幅度腿部动作由 bend 承担（库坑 #2）");
                    }
                }
            }
        }
        return new AxisWalk(axisTicks, axisEasings);
    }

    /** 单轴机械断言：轴存在、相邻帧距 ≤4 tick、无 linear easing（打击轴与段式主轴共用）。 */
    private static void assertAxisDense(String animName, AxisWalk walk, String axis) {
        List<Integer> ticks = walk.ticks().get(axis);
        assertNotNull(ticks, animName + " manifest 声明的主轴 `" + axis + "` 在动画中无关键帧");
        List<Integer> sorted = ticks.stream().sorted().distinct().toList();
        for (int i = 1; i < sorted.size(); i++) {
            assertTrue(sorted.get(i) - sorted.get(i - 1) <= 4,
                animName + " 主轴 `" + axis + "` 帧点 " + sorted.get(i - 1) + "→"
                    + sorted.get(i) + " 间隔超过 4 tick（精度标准 #3 密度红线）");
        }
        Set<String> easings = walk.easings().getOrDefault(axis, Set.of());
        assertFalse(easings.stream().anyMatch(e -> e.equalsIgnoreCase("linear")),
            animName + " 主轴 `" + axis + "` 使用 linear easing（精度标准 #3 禁用）");
    }
}
