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
 *   <tr><td>baomai.full_power_release</td><td>强 / 20t≈1.0s</td><td>+9° / 7t</td></tr>
 *   <tr><td>woliu.turbulence_burst</td><td>中 / 18t≈0.9s</td><td>+6° / 6t</td></tr>
 *   <tr><td>zhenmai.sever_chain</td><td>中 / 14t≈0.7s</td><td>—</td></tr>
 *   <tr><td>anqi.echo_fractal</td><td>弱 / 12t≈0.6s</td><td>—</td></tr>
 * </table>
 *
 * <p><b>⚠️ 服务端权威 CASTING 缺口（本表 4 招里 3 招当前拿不到 juice）</b>：
 * {@link CastFovController} 按 plan §P3 门控硬约束只认服务端权威 {@code cast_sync{phase:casting}}
 * 作为 accepted 凭据（本地预测不算）。而服务端 {@code push_skill_cast_started_sync}
 * （{@code server/src/network/client_request_handler.rs}）在实体上没有 {@code Casting} 组件时
 * 直接 early-return——下列 resolver 全程不插 {@code Casting}（属瞬发招，resolver 内一次结算完，
 * 没有引导窗），故服务端**从不**为它们下发权威 CASTING：
 * <ul>
 *   <li>{@code baomai.full_power_release} —— {@code combat::baomai_v3::skills::cast_full_power_release}</li>
 *   <li>{@code woliu.turbulence_burst} —— {@code combat::woliu_v2::skills::resolve_woliu_v2_skill}</li>
 *   <li>{@code anqi.echo_fractal} —— {@code combat::anqi_v2::resolve_anqi_skill}</li>
 * </ul>
 * 只有 {@code zhenmai.sever_chain}（{@code zhenmai_v2::insert_casting_snapshot}）与走动画事件
 * 路径的 {@code sword_path.heaven_gate}（{@code sword_path::skill_register::insert_casting}）
 * 会下发。本表条目**故意保留**：参数是 plan §P3 定稿，服务端补发权威 CASTING（或把这些瞬发招
 * 也接到动画事件驱动）后即刻生效——**那是服务端/跨端改动，不在本纯 client PR 范围**。
 * 不为了让它们「看起来能用」而放宽 accepted 门控。
 *
 * <p><b>heaven_gate 例外</b>：{@code sword_path.heaven_gate} 的 cast 条时长（cast_ticks=80=4s）
 * 与真实引导窗（到 140t=7s 才 emit release）错开 3s，走 CastState 驱动会让 juice 在举剑蓄力
 * 中途触发、而非劈下那一刻。故它**不在本表**，改由 {@link CastFovController#onAnimPlayed}
 * 动画事件驱动（charge 动画→渐强 / release 动画→最大+FOV），与画面严格对齐。
 */
public final class CastJuiceProfiles {
    /** 抖动强度三档（映射 §P3 表「强/中/弱」；peak 幅度 = 2·intensity 度，见 CameraShakeController）。 */
    public static final float STRONG = 1.2f;
    public static final float MEDIUM = 0.85f;
    public static final float WEAK = 0.5f;

    private static final Map<String, CastJuiceProfile> BY_SKILL = build();

    private CastJuiceProfiles() {
    }

    private static Map<String, CastJuiceProfile> build() {
        Map<String, CastJuiceProfile> m = new LinkedHashMap<>();
        // heaven_gate 不登记——cast 条与引导窗错开 3s，改由动画事件驱动（见类文档 + CastFovController）。
        register(m, new CastJuiceProfile("baomai.full_power_release", STRONG, 20, 9.0f, 7));
        register(m, new CastJuiceProfile("woliu.turbulence_burst", MEDIUM, 18, 6.0f, 6));
        register(m, new CastJuiceProfile("zhenmai.sever_chain", MEDIUM, 14, 0f, 0));
        register(m, new CastJuiceProfile("anqi.echo_fractal", WEAK, 12, 0f, 0));
        return Map.copyOf(m);
    }

    private static void register(Map<String, CastJuiceProfile> m, CastJuiceProfile profile) {
        m.put(profile.skillId(), profile);
    }

    /** 该招的 release juice 参数；未登记（非重型招）返回 null。 */
    public static CastJuiceProfile get(String skillId) {
        return skillId == null ? null : BY_SKILL.get(skillId);
    }

    /** 已登记的重型招 id 集合（测试 pin 用）。 */
    public static Set<String> skillIds() {
        return BY_SKILL.keySet();
    }
}
