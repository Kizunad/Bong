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
 * <table>
 *   <tr><th>招式</th><th>shake</th><th>FOV 脉冲</th></tr>
 *   <tr><td>sword_path.heaven_gate</td><td>强 / 8t</td><td>+6° / 4t</td></tr>
 *   <tr><td>baomai.full_power_release</td><td>强 / 6t</td><td>+4° / 4t</td></tr>
 *   <tr><td>woliu.turbulence_burst</td><td>中 / 6t</td><td>+3° / 4t</td></tr>
 *   <tr><td>zhenmai.sever_chain</td><td>中 / 4t</td><td>—</td></tr>
 *   <tr><td>anqi.echo_fractal</td><td>弱 / 3t</td><td>—</td></tr>
 * </table>
 */
public final class CastJuiceProfiles {
    /** 抖动强度三档（映射 §P3 表「强/中/弱」）。 */
    public static final float STRONG = 0.9f;
    public static final float MEDIUM = 0.6f;
    public static final float WEAK = 0.3f;

    private static final Map<String, CastJuiceProfile> BY_SKILL = build();

    private CastJuiceProfiles() {
    }

    private static Map<String, CastJuiceProfile> build() {
        Map<String, CastJuiceProfile> m = new LinkedHashMap<>();
        register(m, new CastJuiceProfile("sword_path.heaven_gate", STRONG, 8, 6.0f, 4));
        register(m, new CastJuiceProfile("baomai.full_power_release", STRONG, 6, 4.0f, 4));
        register(m, new CastJuiceProfile("woliu.turbulence_burst", MEDIUM, 6, 3.0f, 4));
        register(m, new CastJuiceProfile("zhenmai.sever_chain", MEDIUM, 4, 0f, 0));
        register(m, new CastJuiceProfile("anqi.echo_fractal", WEAK, 3, 0f, 0));
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
