package com.bong.client.combat.juice;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * plan-fpv-cast-av-v1 P3 —— 施法 release juice 按招注册表（§P3 参数表的代码落地）。
 *
 * <p>沉浸式极简：**只重型招登记**，未登记的招 {@link #get} 返回 null → 无 juice。抖动强度
 * 「强/中/弱」映射到 {@link #STRONG}/{@link #MEDIUM}/{@link #WEAK} 三档常量。
 *
 * <p><b>手感取向</b>（用户实机调校）：施法 release 的抖动是**持续震动**（{@code sustained}
 * 包络，见 {@link CameraShakeController}）——大招整段撑满存在感，不是命中的「抖一下」。因此
 * shake 时长按秒计（≥0.5s），FOV punch 也放大到明显能感到（≥6°）。
 *
 * <table>
 *   <tr><th>招式</th><th>shake（持续型）</th><th>FOV punch</th></tr>
 *   <tr><td>zhenmai.sever_chain</td><td>中 / 14t≈0.7s</td><td>—</td></tr>
 * </table>
 *
 * <p><b>🚧 baomai.full_power_release / woliu.turbulence_burst / anqi.echo_fractal 不在本表——
 * 参数已定稿但代码侧不留死注册（PR #1249 三轮返工摘掉，2026-07-27）</b>（plan §P3「未完成欠账」
 * 节，P3 状态为 ⏳）：{@link CastFovController} 按 plan §P3 门控硬约束只认服务端权威
 * {@code cast_sync{phase:casting}} 作为 accepted 凭据（本地预测不算）。而服务端
 * {@code push_skill_cast_started_sync}（{@code server/src/network/client_request_handler.rs}）
 * 在实体上没有 {@code Casting} 组件时直接 early-return——下列 resolver 全程不插 {@code Casting}
 * （属瞬发招，resolver 内一次结算完，没有引导窗），故服务端**从不**为它们下发权威 CASTING：
 * <ul>
 *   <li>{@code baomai.full_power_release} —— {@code combat::baomai_v3::skills::cast_full_power_release}</li>
 *   <li>{@code woliu.turbulence_burst} —— {@code combat::woliu_v2::skills::resolve_woliu_v2_skill}</li>
 *   <li>{@code anqi.echo_fractal} —— {@code combat::anqi_v2::resolve_anqi_skill}</li>
 * </ul>
 * 只有 {@code zhenmai.sever_chain}（{@code zhenmai_v2::insert_casting_snapshot}）与走动画事件
 * 路径的 {@code sword_path.heaven_gate}（{@code sword_path::skill_register::insert_casting}）
 * 会下发。**二轮返工曾把这三条保留为「故意保留」的正式注册项**（参数是 plan 定稿、链路补齐后
 * 即刻生效）；三轮 review reviewer C/D（confidence 99）判定这个处置不成立：本 PR 声称交付的就是
 * P3，留着生产不可达的正式注册项等于把「注册集合」和「生产可达集合」混为一谈，测试也只能证明
 * 「客户端能正确消费一条服务端当前不会生产的报文」，锁不住跨端真实契约。**三轮返工改为摘掉**：
 * 三招完整调参数值（强度档位/时长/FOV，与二轮相同，一字未改）已原样搬进
 * {@code docs/plan-fpv-cast-av-v1.md §P3 参数 amendment}，代码侧对应的 skillId 对 {@link #get}
 * 返回 null（与任何未登记的普通招同一行为）。**恢复条件**：服务端补发权威 CASTING（或把这些
 * 瞬发招也接到动画事件驱动，两条候选路线见 plan §P3「未完成欠账」，需 owner 拍板）后，照 plan
 * 表数值原样 {@code register(...)} 回 {@link #build}——不放宽 accepted 门控去凑「看起来能用」
 * （那是回退掉二轮修掉的 bug）。
 *
 * <p><b>heaven_gate 例外</b>：{@code sword_path.heaven_gate} 的 cast 条时长（cast_ticks=80=4s）
 * 与真实引导窗（到 140t=7s 才 emit release）错开 3s，走 CastState 驱动会让 juice 在举剑蓄力
 * 中途触发、而非劈下那一刻。故它**不在本表**，改由 {@link CastFovController#onAnimPlayed}
 * 动画事件驱动（charge 动画→渐强 / release 动画→最大+FOV），与画面严格对齐。该路径的
 * accepted 门控**不打折**：令牌由 heaven_gate 的权威 CASTING 武装，动画事件只是触发时刻。
 */
public final class CastJuiceProfiles {
    /** 抖动强度三档（映射 §P3 表「强/中/弱」；peak 幅度 = 2·intensity 度，见 CameraShakeController）。 */
    public static final float STRONG = 1.2f;
    public static final float MEDIUM = 0.85f;
    public static final float WEAK = 0.5f;

    private static final Map<String, CastJuiceProfile> BY_SKILL = build();

    /**
     * 测试注入的合成条目（生产恒为空 map；单测单线程实际无竞争，volatile 只为可见性）。
     *
     * <p>{@code CastFovControllerTest} / {@code JuiceConfigTest} / {@code JuiceControlsTest} 用它
     * 给一个<b>不进生产注册集合</b>的技能 id 挂一条 profile，驱动 {@link CastFovController} 与
     * 配置接线的<b>通用</b>状态机（arm/release/interrupt/idle/supersession/terminal/teardown/
     * multiplier）——这与「本注册表的真实内容是否生产可达」是两件事，后者由 {@link #skillIds()}
     * （不含合成条目）单独锁。见 {@link #setSyntheticEntryForTest}。
     */
    private static volatile Map<String, CastJuiceProfile> testOverrides = Map.of();

    private CastJuiceProfiles() {
    }

    private static Map<String, CastJuiceProfile> build() {
        Map<String, CastJuiceProfile> m = new LinkedHashMap<>();
        // heaven_gate 不登记——cast 条与引导窗错开 3s，改由动画事件驱动（见类文档 + CastFovController）。
        // baomai/woliu/anqi 三招参数已定稿但不在此登记：服务端从不下发权威 CASTING，登记了在
        // 当前门控设计下就是死代码（PR #1249 三轮返工摘掉，见类文档「服务端权威 CASTING 缺口」
        // 段）。参数值原样存在 docs/plan-fpv-cast-av-v1.md §P3 参数表，服务端补链后照表恢复。
        register(m, new CastJuiceProfile("zhenmai.sever_chain", MEDIUM, 14, 0f, 0));
        return Map.copyOf(m);
    }

    private static void register(Map<String, CastJuiceProfile> m, CastJuiceProfile profile) {
        m.put(profile.skillId(), profile);
    }

    /** 该招的 release juice 参数；未登记（非重型招）返回 null。 */
    public static CastJuiceProfile get(String skillId) {
        if (skillId == null) {
            return null;
        }
        CastJuiceProfile override = testOverrides.get(skillId);
        return override != null ? override : BY_SKILL.get(skillId);
    }

    /** 已登记的重型招 id 集合（测试 pin 用；不含测试合成条目，见 {@link #setSyntheticEntryForTest}）。 */
    public static Set<String> skillIds() {
        return BY_SKILL.keySet();
    }

    /**
     * 测试专用：注入一条不进生产注册集合的合成 profile（见 {@link #testOverrides} 文档）。
     * 覆盖式（单次调用替换整份 override，不叠加）；{@code skillId} 为 {@code null} 等价于清空。
     */
    static void setSyntheticEntryForTest(String skillId, CastJuiceProfile profile) {
        testOverrides = skillId == null ? Map.of() : Map.of(skillId, profile);
    }

    /** 清空测试合成条目（{@code tearDown} 用，防跨测试类泄漏）。 */
    static void clearSyntheticEntryForTest() {
        testOverrides = Map.of();
    }
}
