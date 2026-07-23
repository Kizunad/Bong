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
 *   <tr><td>sword_path.heaven_gate</td><td>强 / 24t≈1.2s</td><td>+12° / 8t</td></tr>
 *   <tr><td>baomai.full_power_release</td><td>强 / 20t≈1.0s</td><td>+9° / 7t</td></tr>
 *   <tr><td>woliu.turbulence_burst</td><td>中 / 18t≈0.9s</td><td>+6° / 6t</td></tr>
 *   <tr><td>zhenmai.sever_chain</td><td>中 / 14t≈0.7s</td><td>—</td></tr>
 *   <tr><td>anqi.echo_fractal</td><td>弱 / 12t≈0.6s</td><td>—</td></tr>
 * </table>
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
        register(m, new CastJuiceProfile("sword_path.heaven_gate", STRONG, 24, 12.0f, 8));
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
