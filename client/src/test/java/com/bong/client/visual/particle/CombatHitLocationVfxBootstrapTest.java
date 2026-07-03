package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-combat-hit-location-v1 P3 — 锁死部位差异视听反馈的三个新增 event_id
 * （头部暴击 burst / 四肢血色三线 / 腿伤血渍 decal）与 server emit 端
 * {@code gameplay_vfx::COMBAT_HIT_HEAD_CRIT / COMBAT_HIT_LIMB / COMBAT_LEG_WOUND_DECAL}
 * 逐字符对齐 + 全部注册，胸/腹/背命中仍走既有 {@code combat_hit} 不受影响。
 *
 * <p>漏注册任一路由 → server emit 的对应事件视听静默丢失（孤岛）。本测试在编译期外兜底。
 */
public class CombatHitLocationVfxBootstrapTest {
    @Test
    void bootstrapRegistersAllBodyPartRoutes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(CombatHitDirectionPlayer.HIT),
            "通用胸/腹/背命中 event_id 必须仍注册，本次改动不应破坏既有 combat_hit"
        );
        assertTrue(
            VfxRegistry.instance().contains(CombatHitDirectionPlayer.HEAD_CRIT),
            "头部命中 event_id 必须注册，否则 server emit 的 bong:combat_hit_head_crit 视听静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(CombatHitDirectionPlayer.LIMB),
            "四肢命中 event_id 必须注册，否则 server emit 的 bong:combat_hit_limb 视听静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(LegWoundBloodDecalPlayer.EVENT_ID),
            "腿伤血渍 decal event_id 必须注册，否则 server emit 的 bong:combat_leg_wound_decal 视听静默丢失"
        );
    }

    /** event_id 字面值必须与 server 常量逐字符一致（gameplay_vfx.rs COMBAT_HIT_* / COMBAT_LEG_WOUND_DECAL）。 */
    @Test
    void eventIdsMatchServerEmitConstants() {
        assertEquals("bong:combat_hit", CombatHitDirectionPlayer.HIT.toString());
        assertEquals("bong:combat_hit_head_crit", CombatHitDirectionPlayer.HEAD_CRIT.toString());
        assertEquals("bong:combat_hit_limb", CombatHitDirectionPlayer.LIMB.toString());
        assertEquals("bong:combat_leg_wound_decal", LegWoundBloodDecalPlayer.EVENT_ID.toString());
    }

    /** 四个 event_id 必须互不相同——否则不同部位的反馈会退化派发到同一路径。 */
    @Test
    void eventIdsAreMutuallyDistinct() {
        assertNotEquals(CombatHitDirectionPlayer.HIT, CombatHitDirectionPlayer.HEAD_CRIT);
        assertNotEquals(CombatHitDirectionPlayer.HIT, CombatHitDirectionPlayer.LIMB);
        assertNotEquals(CombatHitDirectionPlayer.HEAD_CRIT, CombatHitDirectionPlayer.LIMB);
        assertNotEquals(CombatHitDirectionPlayer.HIT, LegWoundBloodDecalPlayer.EVENT_ID);
        assertNotEquals(CombatHitDirectionPlayer.HEAD_CRIT, LegWoundBloodDecalPlayer.EVENT_ID);
        assertNotEquals(CombatHitDirectionPlayer.LIMB, LegWoundBloodDecalPlayer.EVENT_ID);
    }
}
