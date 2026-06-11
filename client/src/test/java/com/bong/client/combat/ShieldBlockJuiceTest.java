package com.bong.client.combat;

import com.bong.client.animation.BongAnimations;
import com.bong.client.combat.juice.CombatJuiceEvent;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.juice.CombatJuiceTier;
import com.bong.client.combat.juice.CombatSchool;
import com.bong.client.combat.juice.HitStopController;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Arrays;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P4：盾格挡 CombatJuice 差异化测试。
 *
 * <p>覆盖：
 * <ul>
 *   <li>SHIELD_BLOCK enum 存在且独立（不等于 PARRY）</li>
 *   <li>格挡命中发 SHIELD_BLOCK 非 PARRY</li>
 *   <li>SHIELD_BLOCK juice 无屏幕 overlay（区别于 PARRY 的闪烁）</li>
 *   <li>SHIELD_BLOCK 产生轻量 hit-stop（LIGHT tier）</li>
 *   <li>HURT_STAGGER 优先级（2000）> PARRY_BLOCK（1000）守护</li>
 *   <li>CombatEventHandler "shield_block" wire → SHIELD_BLOCK 路由</li>
 * </ul>
 */
class ShieldBlockJuiceTest {

    @BeforeEach
    @AfterEach
    void resetState() {
        CombatJuiceSystem.resetForTests();
    }

    // ── SHIELD_BLOCK 枚举独立性 ───────────────────────────────────────────

    @Test
    void shieldBlock_kindExists_asDistinctEnumValue() {
        boolean found = Arrays.stream(CombatJuiceEvent.Kind.values())
            .anyMatch(k -> k == CombatJuiceEvent.Kind.SHIELD_BLOCK);
        assertTrue(found,
            "CombatJuiceEvent.Kind.SHIELD_BLOCK 应存在（plan-shield-block-v1 §P4 新增），实际 enum 中未找到");
    }

    @Test
    void shieldBlock_kindIsNotParry() {
        assertNotEquals(
            CombatJuiceEvent.Kind.PARRY,
            CombatJuiceEvent.Kind.SHIELD_BLOCK,
            "SHIELD_BLOCK 不应复用 PARRY（PARRY 音效硬编码剑击声，复用会播错音）"
        );
    }

    @Test
    void shieldBlock_kindIsNotPerfectParry() {
        assertNotEquals(
            CombatJuiceEvent.Kind.PERFECT_PARRY,
            CombatJuiceEvent.Kind.SHIELD_BLOCK,
            "SHIELD_BLOCK 不应与 PERFECT_PARRY 相同"
        );
    }

    // ── SHIELD_BLOCK juice 系统行为 ───────────────────────────────────────

    @Test
    void shieldBlock_accept_returnsHandledCommand() {
        CombatJuiceEvent event = shieldBlockEvent("attacker", "defender", 1_000L);
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 1_000L);

        assertNotNull(command.event(),
            "SHIELD_BLOCK juice 应产出非空 command.event()，实际 null");
        assertEquals(
            CombatJuiceEvent.Kind.SHIELD_BLOCK,
            command.event().kind(),
            "command.event().kind() 应为 SHIELD_BLOCK，实际=" + command.event().kind()
        );
    }

    @Test
    void shieldBlock_producesHitStop() {
        CombatJuiceEvent event = shieldBlockEvent("attacker", "defender", 2_000L);
        CombatJuiceSystem.accept(event, 2_000L);

        // SHIELD_BLOCK 走 LIGHT tier hit-stop，应对 defender 有冻结
        assertTrue(
            HitStopController.isFrozen("defender", 2_001L) || HitStopController.remainingTicks("defender", 2_000L) > 0,
            "SHIELD_BLOCK 应产生 hit-stop（LIGHT tier），defender 在命中后 1ms 应仍被冻结或有剩余 ticks"
        );
    }

    @Test
    void shieldBlock_noScreenOverlay() {
        CombatJuiceEvent event = shieldBlockEvent("attacker", "defender", 3_000L);
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 3_000L);

        assertFalse(
            command.overlay().activeAt(3_000L),
            "SHIELD_BLOCK 不应产生屏幕 overlay（区别于 PARRY 的屏幕闪烁），实际 overlay 仍 active"
        );
    }

    @Test
    void shieldBlock_noParryPlan() {
        CombatJuiceEvent event = shieldBlockEvent("attacker", "defender", 4_000L);
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 4_000L);

        assertNull(
            command.parry(),
            "SHIELD_BLOCK 不应产生 ParryPlan（仅 PARRY / PERFECT_PARRY 才有），实际 parry=" + command.parry()
        );
    }

    // ── HURT_STAGGER > PARRY_BLOCK 优先级守护 ────────────────────────────

    @Test
    void animationPriority_hurtStagger_greaterThan_parryBlock() {
        assertTrue(
            BongAnimations.PRIORITY_HIT_RECOIL > BongAnimations.PRIORITY_COMBAT,
            "HURT_STAGGER 优先级(" + BongAnimations.PRIORITY_HIT_RECOIL
                + ") 应 > PARRY_BLOCK 优先级(" + BongAnimations.PRIORITY_COMBAT
                + ")；格挡命中同时受击硬直时，HURT_STAGGER 应盖过 PARRY_BLOCK recoil"
        );
    }

    @Test
    void animationPriority_hurtStagger_is2000() {
        assertEquals(
            2000,
            BongAnimations.PRIORITY_HIT_RECOIL,
            "HURT_STAGGER 优先级应为 2000（server HIT_RECOIL_PRIORITY 常量），实际=" + BongAnimations.PRIORITY_HIT_RECOIL
        );
    }

    @Test
    void animationPriority_parryBlock_is1000() {
        assertEquals(
            1000,
            BongAnimations.PRIORITY_COMBAT,
            "PARRY_BLOCK 战斗层优先级应为 1000（server COMBAT_PRIORITY 常量），实际=" + BongAnimations.PRIORITY_COMBAT
        );
    }

    // ── CombatEventHandler wire 路由 ─────────────────────────────────────

    @Test
    void wireKind_shieldBlock_routesToShieldBlock() {
        // "shield_block" wire kind 应路由到 SHIELD_BLOCK，不应路由到 PARRY
        // 通过 end-to-end ServerDataRouter 路由验证
        String json = """
            {"v":1,"type":"combat_event","events":[
              {"kind":"shield_block","amount":8,"target_uuid":"defender"}
            ]}""";
        com.bong.client.combat.store.DamageFloaterStore.resetForTests();
        var router = com.bong.client.network.ServerDataRouter.createDefault();
        router.route(json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);

        CombatJuiceSystem.LastCommand last = CombatJuiceSystem.lastCommand();
        assertNotNull(last, "combat_event(shield_block) 应被路由");
        assertNotNull(last.event(), "combat_event(shield_block) 应进入 CombatJuiceSystem");
        assertEquals(
            CombatJuiceEvent.Kind.SHIELD_BLOCK,
            last.event().kind(),
            "wire kind='shield_block' 应路由到 SHIELD_BLOCK，实际 kind=" + last.event().kind()
        );
    }

    @Test
    void wireKind_block_stillRoutesToParry() {
        // "block" wire kind（JieMai 截脉格挡）仍应路由 PARRY，不受 SHIELD_BLOCK 新增影响
        String json = """
            {"v":1,"type":"combat_event","events":[
              {"kind":"block","amount":5,"target_uuid":"defender"}
            ]}""";
        com.bong.client.combat.store.DamageFloaterStore.resetForTests();
        var router = com.bong.client.network.ServerDataRouter.createDefault();
        router.route(json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);

        CombatJuiceSystem.LastCommand last = CombatJuiceSystem.lastCommand();
        assertNotNull(last);
        assertNotNull(last.event());
        assertEquals(
            CombatJuiceEvent.Kind.PARRY,
            last.event().kind(),
            "wire kind='block' 应仍路由到 PARRY（JieMai 截脉格挡兼容路径），实际 kind=" + last.event().kind()
        );
    }

    // ── helpers ────────────────────────────────────────────────────────────

    private static CombatJuiceEvent shieldBlockEvent(String attackerUuid, String defenderUuid, long nowMs) {
        return new CombatJuiceEvent(
            CombatJuiceEvent.Kind.SHIELD_BLOCK,
            CombatSchool.GENERIC,
            CombatJuiceTier.LIGHT,
            attackerUuid,
            defenderUuid,
            "",
            "",
            0.0,
            1.0,
            false,
            nowMs
        );
    }
}
