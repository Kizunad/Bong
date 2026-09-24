package com.bong.client.animation;

import net.minecraft.util.Identifier;

import java.util.Map;

/**
 * 手持物驱动的步态**变体**解析——纯函数，不碰 MC 运行时，便于饱和单测。
 *
 * <p>{@link GaitSelector} 只回答"哪一档"（走 / 慢跑 / 冲刺 / 瞬步），本类回答"这一档，
 * 手里拿着这个东西时该播哪一条"。两件事分开是因为档位判定与手持物完全正交：换武器
 * 不该改变档位逻辑，改档位阈值也不该动变体表。
 *
 * <p><b>为什么不把手臂写进全局 {@code lower_*}</b>：那四条是所有武器与空手共用的
 * （{@link LowerBodyGaitController}），把持刀手型写进去，拿剑拿棍空手的人也会摆出
 * 握采药刀的姿势。变体表让"携行手型"只在真的拿着那件东西时生效。
 *
 * <p><b>变体动画可以写手臂</b>：本通道是 {@link AnimationLayerManager.Channel#LOWER_BODY}
 * （priority 500），招式在 {@code UPPER_BODY}（1000），施法时手臂由上层接管，不会打架。
 * 但仍**不写 torso / head**——那两处要留给"边走边看四周"与招式的躯干拧转
 * （`gen_herb_knife_carry_gait.assert_carry_only` 在生成侧挡住）。
 *
 * <p>查不到就回落到全局步态。所以只覆盖了 {@code WALK} / {@code SPRINT} 的采药刀
 * 拿着慢跑时，走的仍是全局 {@code lower_jog}、手臂交还上层——**缺变体是安全的降级，
 * 不是断链**。
 */
public final class GaitVariants {
    /** 凡铁采药刀的 server 侧 template_id（{@code EquippedWeapon.templateId}）。 */
    public static final String HERB_KNIFE_IRON = "herb_knife_iron";

    /** {@code template_id → (档位 → 变体动画)}。查不到的组合回落到 {@code gait.animId()}。 */
    private static final Map<String, Map<GaitSelector.Gait, Identifier>> BY_TEMPLATE = Map.of(
        HERB_KNIFE_IRON, Map.of(
            GaitSelector.Gait.WALK, new Identifier("bong", "herb_knife_carry_walk")
        )
    );

    private GaitVariants() {
    }

    /**
     * 解析这一档在当前手持物下实际该播的动画。
     *
     * @param gait           {@link GaitSelector#select} 的结果
     * @param heldTemplateId 主手 Bong 物品的 template_id；空手 / 非 Bong 物品传 {@code null}
     * @return 变体动画 id；无变体时返回该档的全局动画；{@code NONE} 档返回 {@code null}
     */
    public static Identifier resolve(GaitSelector.Gait gait, String heldTemplateId) {
        if (gait == null) {
            return null;
        }
        Identifier base = gait.animId();
        if (base == null || heldTemplateId == null) {
            return base;
        }
        Map<GaitSelector.Gait, Identifier> byGait = BY_TEMPLATE.get(heldTemplateId.trim());
        if (byGait == null) {
            return base;
        }
        return byGait.getOrDefault(gait, base);
    }

    /** 该 template_id 有没有登记任何变体（供测试与诊断）。 */
    public static boolean hasVariants(String heldTemplateId) {
        return heldTemplateId != null && BY_TEMPLATE.containsKey(heldTemplateId.trim());
    }
}
