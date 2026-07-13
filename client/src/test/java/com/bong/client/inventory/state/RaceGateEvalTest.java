package com.bong.client.inventory.state;

import com.bong.client.inventory.model.RaceGate;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P3c — {@link RaceGateEval} 饱和判定测试。
 *
 * <p>覆盖三档 gate（any/humanoid/species）× 判定域身份（人形/非人形、匹配/不匹配种族）
 * 全矩阵 + fail-closed（身份未知）+ 装备域(当前形态)与功法域(本体)不同轴 + store 缺条目
 * 恒放行。断言纯 {@link RaceGateEval.Decision} 三态（放行 / 不匹配置灰 / 身份未知置灰），
 * 失败信息带修复线索。
 */
class RaceGateEvalTest {

    private static final RaceGate ANY = new RaceGate("any", List.of());
    private static final RaceGate HUMANOID = new RaceGate("humanoid", List.of());
    private static final RaceGate WHALE = new RaceGate("species", List.of("whale"));

    @BeforeEach
    void setUp() {
        RaceGateMetaStore.resetForTests();
        PlayerRaceIdentityStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        RaceGateMetaStore.resetForTests();
        PlayerRaceIdentityStore.resetForTests();
    }

    // ─── 纯 evaluate 核心：三档 × 身份矩阵 ───────────────────────────

    @Test
    void anyGateAlwaysAllowsRegardlessOfIdentity() {
        // any 恒放行，无论身份是否已知 / 是什么。
        assertEquals(RaceGateEval.Decision.ALLOW,
            RaceGateEval.evaluate(ANY, false, "", false),
            "any 门在身份未知时也应放行（any 不受 fail-closed 约束）");
        assertEquals(RaceGateEval.Decision.ALLOW,
            RaceGateEval.evaluate(ANY, true, "whale", false));
        assertEquals(RaceGateEval.Decision.ALLOW,
            RaceGateEval.evaluate(null, false, "", false),
            "null gate（表内查不到=any）同样恒放行");
    }

    @Test
    void humanoidGateAllowsHumanoidBlocksNonHumanoid() {
        assertEquals(RaceGateEval.Decision.ALLOW,
            RaceGateEval.evaluate(HUMANOID, true, "human", true),
            "humanoid 门 + 人形身份应放行");
        assertEquals(RaceGateEval.Decision.BLOCK_MISMATCH,
            RaceGateEval.evaluate(HUMANOID, true, "whale", false),
            "humanoid 门 + 非人形身份应置灰（不匹配）");
    }

    @Test
    void speciesGateAllowsListedSpeciesBlocksOthers() {
        assertEquals(RaceGateEval.Decision.ALLOW,
            RaceGateEval.evaluate(WHALE, true, "whale", false),
            "species([whale]) 门 + whale 身份应放行（不看是否人形）");
        assertEquals(RaceGateEval.Decision.BLOCK_MISMATCH,
            RaceGateEval.evaluate(WHALE, true, "human", true),
            "species([whale]) 门 + human 身份应置灰");
    }

    @Test
    void failClosedWhenIdentityUnknownAndGatePresent() {
        assertEquals(RaceGateEval.Decision.BLOCK_IDENTITY_UNKNOWN,
            RaceGateEval.evaluate(HUMANOID, false, "", false),
            "有 gate 但身份未知 → fail-closed 置灰（宁可错灰不可错放）");
        assertEquals(RaceGateEval.Decision.BLOCK_IDENTITY_UNKNOWN,
            RaceGateEval.evaluate(WHALE, false, "", false));
    }

    @Test
    void unknownKindBlockedSentinelAlwaysBlocksEvenWhenIdentityKnownAndHumanoid() {
        // P3 opus verify LOW 项：RaceGateMetaHandler 对未知 gate.kind 存入
        // RaceGate.blocked() 哨兵而非跳过。此处锁死 RaceGateEval 对该哨兵的判定——
        // 哨兵不是 any（isAny()==false），身份已知时必须落 BLOCK_MISMATCH（不是 ALLOW），
        // 无论传入的身份是人形还是非人形、raceId 是什么，都不该放行。
        RaceGate blocked = com.bong.client.inventory.model.RaceGate.blocked();
        assertFalse(blocked.isAny(), "blocked() 哨兵不应被判定为 any");
        assertEquals(RaceGateEval.Decision.BLOCK_MISMATCH,
            RaceGateEval.evaluate(blocked, true, "human", true),
            "未知 kind 哨兵 + 人形身份已知：仍应置灰（不匹配），不能放行");
        assertEquals(RaceGateEval.Decision.BLOCK_MISMATCH,
            RaceGateEval.evaluate(blocked, true, "whale", false),
            "未知 kind 哨兵 + 非人形身份已知：仍应置灰（不匹配），不能放行");
        assertEquals(RaceGateEval.Decision.BLOCK_IDENTITY_UNKNOWN,
            RaceGateEval.evaluate(blocked, false, "", false),
            "未知 kind 哨兵 + 身份未知：fail-closed 置灰（与其它有 gate 条目一致）");
    }

    @Test
    void decisionBlockedHelperTrueForBothBlockKinds() {
        assertTrue(RaceGateEval.Decision.BLOCK_MISMATCH.blocked());
        assertTrue(RaceGateEval.Decision.BLOCK_IDENTITY_UNKNOWN.blocked());
        assertFalse(RaceGateEval.Decision.ALLOW.blocked());
    }

    // ─── 装备门（当前形态身份） ──────────────────────────────────────

    @Test
    void itemGateUsesFormIdentityNotIntrinsic() {
        // 本体 human(人形) 但易形为 whale(非人形)：human-only 物品应按**当前形态**置灰。
        PlayerRaceIdentityStore.replace("human", "whale", "whale", true, false);
        RaceGateMetaStore.replace(Map.of("mask", HUMANOID), Map.of());

        assertTrue(RaceGateEval.isItemBlocked("mask"),
            "装备门判当前形态：易形成非人形 whale 后 human-only 面具应置灰，"
                + "即便本体是人形（装备域用 form_*，不用 intrinsic_*）");
    }

    @Test
    void itemGateAllowsWhenFormMatches() {
        PlayerRaceIdentityStore.replace("human", "human", "humanoid", true, true);
        RaceGateMetaStore.replace(Map.of("mask", HUMANOID), Map.of());
        assertFalse(RaceGateEval.isItemBlocked("mask"),
            "未易形（form=人形）时 human-only 面具应放行");
    }

    @Test
    void itemWithoutGateAlwaysAllowed() {
        PlayerRaceIdentityStore.replace("human", "whale", "whale", true, false);
        RaceGateMetaStore.replace(Map.of("mask", HUMANOID), Map.of());
        assertFalse(RaceGateEval.isItemBlocked("plain_rock"),
            "表内无条目的物品 = any = 恒放行，哪怕当前形态是非人形");
    }

    @Test
    void itemFailClosedWhenFormIdentityUnknown() {
        // 收到 meta 但尚未收到身份快照（form_race_id 空）：有 gate 的物品置灰。
        RaceGateMetaStore.replace(Map.of("mask", HUMANOID), Map.of());
        assertTrue(RaceGateEval.isItemBlocked("mask"),
            "身份未到（首帧乱序）时有 gate 的物品 fail-closed 置灰");
        assertFalse(RaceGateEval.isItemBlocked("plain_rock"),
            "身份未到时无 gate 物品仍放行（fail-closed 只作用于有 gate 条目）");
    }

    // ─── 功法门（本体身份） ──────────────────────────────────────────

    @Test
    void techniqueGateUsesIntrinsicIdentityNotForm() {
        // 本体 human(人形)，易形成 whale(非人形)：人形专属功法应按**本体**放行（功法不随易形废）。
        PlayerRaceIdentityStore.replace("human", "whale", "whale", true, false);
        RaceGateMetaStore.replace(Map.of(), Map.of("sword.cleave", HUMANOID));
        assertFalse(RaceGateEval.isTechniqueBlocked("sword.cleave"),
            "功法门判本体：本体人形则人形功法放行，即便当前易形成非人形（功法域用 intrinsic_*）");
    }

    @Test
    void techniqueGateBlocksWhenIntrinsicMismatch() {
        // 本体就是非人形 whale：人形专属功法应置灰。
        PlayerRaceIdentityStore.replace("whale", "whale", "whale", false, false);
        RaceGateMetaStore.replace(Map.of(), Map.of("sword.cleave", HUMANOID));
        assertTrue(RaceGateEval.isTechniqueBlocked("sword.cleave"),
            "本体非人形（whale）时人形专属功法应置灰");
    }

    @Test
    void techniqueFailClosedWhenIntrinsicIdentityUnknown() {
        RaceGateMetaStore.replace(Map.of(), Map.of("sword.cleave", HUMANOID));
        assertTrue(RaceGateEval.isTechniqueBlocked("sword.cleave"),
            "本体身份未到时有 gate 的功法 fail-closed 置灰");
    }

    @Test
    void techniqueWithoutGateAlwaysAllowed() {
        PlayerRaceIdentityStore.replace("whale", "whale", "whale", false, false);
        RaceGateMetaStore.replace(Map.of(), Map.of("sword.cleave", HUMANOID));
        assertFalse(RaceGateEval.isTechniqueBlocked("flying_sword.control"),
            "表内无条目的功法（如 flying_sword 神识驱动 = Any）恒放行，即便本体非人形");
    }

    // ─── 状态转换：易形改变判定结果 ─────────────────────────────────

    @Test
    void morphTransitionFlipsItemDecisionButNotTechnique() {
        RaceGateMetaStore.replace(Map.of("mask", HUMANOID), Map.of("sword.cleave", HUMANOID));

        // 未易形：human 本体 + human 形态。
        PlayerRaceIdentityStore.replace("human", "human", "humanoid", true, true);
        assertFalse(RaceGateEval.isItemBlocked("mask"), "未易形：面具可穿");
        assertFalse(RaceGateEval.isTechniqueBlocked("sword.cleave"), "未易形：功法可用");

        // 易形成 whale（非人形形态，本体仍 human 人形）。
        PlayerRaceIdentityStore.replace("human", "whale", "whale", true, false);
        assertTrue(RaceGateEval.isItemBlocked("mask"),
            "易形后装备门（当前形态非人形）→ 面具置灰");
        assertFalse(RaceGateEval.isTechniqueBlocked("sword.cleave"),
            "易形后功法门（本体仍人形）→ 功法仍可用（不随易形废）");
    }
}
