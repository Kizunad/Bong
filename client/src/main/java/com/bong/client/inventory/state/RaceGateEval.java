package com.bong.client.inventory.state;

import com.bong.client.inventory.model.RaceGate;

/**
 * plan-race-system-v1 P3c — 种族门置灰判定（装备格 / 功法条目）。
 *
 * <p>与 server `validate_equip_to` / 功法习得门**同结论**：client 只做预览置灰，
 * server 权威——被绕过时 server 仍拒 + toast 兜底。
 *
 * <p><b>两域不同轴</b>（决议 §8.1 #5/#6）：
 * <ul>
 *   <li><b>装备门</b>用**当前形态**身份（{@code form_race_id} / {@code form_is_humanoid}）。</li>
 *   <li><b>功法门</b>用**本体**身份（{@code race_id} / {@code intrinsic_is_humanoid}）。</li>
 * </ul>
 *
 * <p><b>fail-closed</b>：条目**有 gate**（非 any）但对应身份尚未收到权威快照
 * （{@link PlayerRaceIdentityStore} 首帧乱序 / 未到）时，一律置灰（宁可错灰不可错放）；
 * 拿到身份后监听者重刷放开。gate 为 any（表内查不到）的条目**恒放行**，与身份是否已知无关。
 */
public final class RaceGateEval {

    private RaceGateEval() {}

    /** 纯判定结果，供测试断言三态（放行 / 置灰-不匹配 / 置灰-身份未知）不必猜原因。 */
    public enum Decision {
        /** 放行（无 gate 或身份匹配）。 */
        ALLOW,
        /** 置灰：有 gate 且当前身份明确不匹配。 */
        BLOCK_MISMATCH,
        /** 置灰：有 gate 但身份未知（fail-closed）。 */
        BLOCK_IDENTITY_UNKNOWN;

        public boolean blocked() {
            return this != ALLOW;
        }
    }

    /**
     * 纯判定核心（无 store 依赖，供饱和测试直接喂三档 gate × 身份矩阵）。
     *
     * @param gate           条目的种族门；{@code null} = any = 恒放行
     * @param identityKnown  对应域身份是否已收到权威快照（fail-closed 判据）
     * @param raceId         判定域种族 id（species 档比对）
     * @param isHumanoid     判定域是否人形（humanoid 档比对）
     */
    public static Decision evaluate(RaceGate gate, boolean identityKnown, String raceId, boolean isHumanoid) {
        if (gate == null || gate.isAny()) {
            return Decision.ALLOW;
        }
        if (!identityKnown) {
            return Decision.BLOCK_IDENTITY_UNKNOWN;
        }
        return gate.allows(raceId, isHumanoid) ? Decision.ALLOW : Decision.BLOCK_MISMATCH;
    }

    /**
     * 装备门：某 item 能否被**当前形态**穿戴。读 {@link RaceGateMetaStore} +
     * {@link PlayerRaceIdentityStore} 的当前形态身份。
     */
    public static Decision evaluateItem(String templateId) {
        RaceGate gate = RaceGateMetaStore.gateForItem(templateId);
        return evaluate(
            gate,
            PlayerRaceIdentityStore.formIdentityKnown(),
            PlayerRaceIdentityStore.formRaceId(),
            PlayerRaceIdentityStore.formIsHumanoid()
        );
    }

    /** 装备门便捷布尔：是否应置灰该 item。 */
    public static boolean isItemBlocked(String templateId) {
        return evaluateItem(templateId).blocked();
    }

    /**
     * 功法门：某 technique 能否被**本体**习得/施放。读 {@link RaceGateMetaStore} +
     * {@link PlayerRaceIdentityStore} 的本体身份。
     */
    public static Decision evaluateTechnique(String skillId) {
        RaceGate gate = RaceGateMetaStore.gateForTechnique(skillId);
        return evaluate(
            gate,
            PlayerRaceIdentityStore.intrinsicIdentityKnown(),
            PlayerRaceIdentityStore.raceId(),
            PlayerRaceIdentityStore.intrinsicIsHumanoid()
        );
    }

    /** 功法门便捷布尔：是否应置灰该 technique。 */
    public static boolean isTechniqueBlocked(String skillId) {
        return evaluateTechnique(skillId).blocked();
    }
}
