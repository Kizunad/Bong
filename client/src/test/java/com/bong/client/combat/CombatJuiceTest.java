package com.bong.client.combat;

import com.bong.client.combat.juice.CameraShakeController;
import com.bong.client.combat.juice.CombatJuiceCalibration;
import com.bong.client.combat.juice.CombatJuiceEvent;
import com.bong.client.combat.juice.CombatJuiceProfile;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.juice.CombatJuiceTier;
import com.bong.client.combat.juice.CombatSchool;
import com.bong.client.combat.juice.EntityTintController;
import com.bong.client.combat.juice.HitStopController;
import com.bong.client.combat.juice.KillJuiceController;
import com.bong.client.combat.juice.ParryDodgeJuicePlanner;
import com.bong.client.combat.juice.WoundWorldVisualPlanner;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Field;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.*;

class CombatJuiceTest {
    @BeforeEach
    @AfterEach
    void resetState() {
        CombatJuiceSystem.resetForTests();
    }

    @Test
    void juice_profile_selects_by_tier() {
        CombatJuiceProfile critical = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        assertEquals(10, critical.hitStopTicks());
        assertEquals(0.90f, critical.shakeIntensity(), 0.001f);
        assertEquals(0xFFB87333, critical.qiColorArgb());

        CombatJuiceProfile poison = CombatJuiceProfile.select(CombatSchool.DUGU, CombatJuiceTier.CRITICAL);
        assertEquals(2, poison.hitStopTicks(), "dugu critical should stay low-impact but visible");
        assertEquals(45, poison.tintDurationTicks(), "dugu uses long invasive tint instead of impact shake");

        assertEquals(21, CombatJuiceProfile.profiles().size(), "7 schools x 3 tiers must be configured");
    }

    @Test
    void hit_stop_freezes_entity() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.HEAVY);
        HitStopController.request("attacker", "target", profile, 1_000L);

        assertEquals(6, HitStopController.remainingTicks("target", 1_000L), "expected defender to receive full heavy hit-stop budget because target was hit, actual remaining ticks differed");
        assertEquals(3, HitStopController.remainingTicks("attacker", 1_000L), "expected attacker to receive half heavy hit-stop budget because local swing recovery is shorter, actual remaining ticks differed");
        assertTrue(HitStopController.isFrozen("target", 1_100L), "expected target to remain frozen 100ms into a 6 tick freeze because duration is 300ms, actual was unfrozen");
    }

    @Test
    void hit_stop_attacker_ticks_floor_half_budget() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.GENERIC, CombatJuiceTier.HEAVY);
        HitStopController.request("attacker", "target", profile, 1_000L);

        assertEquals(5, HitStopController.remainingTicks("target", 1_000L), "expected generic heavy defender freeze to use the full 5 tick profile budget, actual remaining ticks differed");
        assertEquals(2, HitStopController.remainingTicks("attacker", 1_000L), "expected attacker freeze to floor half of 5 ticks to 2 because attacker recovery must not exceed design budget, actual remaining ticks differed");
    }

    @Test
    void shake_direction_perpendicular() {
        double[] normal = CameraShakeController.perpendicular(1.0, 0.0, false);
        assertEquals(0.0, normal[0], 0.0001);
        assertEquals(1.0, normal[1], 0.0001);

        double[] reverse = CameraShakeController.perpendicular(1.0, 0.0, true);
        assertEquals(0.0, reverse[0], 0.0001);
        assertEquals(-1.0, reverse[1], 0.0001);
    }

    @Test
    void shake_decays_linearly() {
        CameraShakeController.Shake shake = new CameraShakeController.Shake(
            1.0f, 10, 1.0, 0.0, false, CameraShakeController.Envelope.DECAY, 1_000L);
        assertEquals(1.0, shake.remainingRatioAt(1_000L), 0.0001);
        assertEquals(0.5, shake.remainingRatioAt(1_250L), 0.0001);
        assertEquals(0.0, shake.remainingRatioAt(1_500L), 0.0001);
    }

    @Test
    void sustained_shake_holds_full_then_releases() {
        // SUSTAIN 包络（施法 release 用）：前 70% 维持满幅 1.0，末 30% 线性收束到 0，
        // 与线性「抖一下」DECAY 分叉——这是「持续震动」的手感来源。
        CameraShakeController.Shake s = new CameraShakeController.Shake(
            1.0f, 20, 1.0, 0.0, false, CameraShakeController.Envelope.SUSTAIN, 1_000L);
        long dur = 20 * 50;  // 1000ms
        assertEquals(1.0, s.envelopeAt(1_000L), 1e-9, "t=0 满幅");
        assertEquals(1.0, s.envelopeAt(1_000L + dur / 2), 1e-9, "50% 仍满幅（线性会是 0.5）");
        assertEquals(1.0, s.envelopeAt(1_000L + (long) (dur * 0.7)), 1e-9, "70% 边界仍满幅");
        assertEquals(0.5, s.envelopeAt(1_000L + (long) (dur * 0.85)), 1e-9, "85% → 收束一半");
        assertEquals(0.0, s.envelopeAt(1_000L + dur), 1e-9, "结束归 0");

        // DECAY 同参数在 50% 处应为线性 0.5，证明包络确实按 Envelope 分叉。
        CameraShakeController.Shake lin = new CameraShakeController.Shake(
            1.0f, 20, 1.0, 0.0, false, CameraShakeController.Envelope.DECAY, 1_000L);
        assertEquals(0.5, lin.envelopeAt(1_000L + dur / 2), 1e-9, "DECAY 50% = 线性 0.5");
    }

    @Test
    void crescendo_shake_ramps_up_then_holds_full() {
        // CRESCENDO 包络（蓄力渐强用）：幅度由 0 线性爬到满（前 90%），其后维持满幅撑到
        // release 顶替——与 SUSTAIN（起手即满）、DECAY（起手即满后衰减）都不同。
        CameraShakeController.Shake c = new CameraShakeController.Shake(
            1.0f, 20, 1.0, 0.0, false, CameraShakeController.Envelope.CRESCENDO, 1_000L);
        long dur = 20 * 50;  // 1000ms
        assertEquals(0.0, c.envelopeAt(1_000L), 1e-9, "t=0 幅度为 0（蓄力起手无震动）");
        assertEquals(0.5, c.envelopeAt(1_000L + (long) (dur * 0.45)), 1e-9, "45% → 爬到一半（0.45/0.9）");
        assertEquals(1.0, c.envelopeAt(1_000L + (long) (dur * 0.9)), 1e-9, "90% → 爬满");
        assertEquals(1.0, c.envelopeAt(1_000L + (long) (dur * 0.95)), 1e-9, "95% → 维持满幅（撑到 release）");
    }

    @Test
    void qi_collision_selects_school_color() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.QI_COLLISION,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            2_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 2_000L);

        assertEquals(0xFF4682B4, command.profile().qiColorArgb());
        assertEquals(0xFF4682B4, command.tint().argb());
    }

    @Test
    void entity_tint_lerp_back() {
        EntityTintController.Tint tint =
            EntityTintController.trigger("target", CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.LIGHT), 1_000L);

        assertEquals(0.4f, tint.alphaAt(1_000L), 0.001f);
        assertEquals(0.2f, tint.alphaAt(1_150L), 0.001f);
        assertEquals(0.0f, tint.alphaAt(1_300L), 0.001f);
    }

    @Test
    void full_charge_max_juice() {
        CombatJuiceEvent event = CombatJuiceEvent.hit(
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            0.0,
            1.0,
            3_000L
        );
        event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.FULL_CHARGE,
            event.school(),
            event.tier(),
            event.attackerUuid(),
            event.targetUuid(),
            "",
            "",
            event.directionX(),
            event.directionZ(),
            false,
            event.receivedAtMs()
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 3_000L);

        assertEquals(CombatJuiceTier.CRITICAL, command.profile().tier());
        assertEquals(10, HitStopController.remainingTicks("target", 3_000L));
        assertTrue(command.overlay().activeAt(3_000L));
    }

    @Test
    void full_charge_alias_infers_heavy_tier_without_explicit_tier() {
        assertEquals(
            CombatJuiceTier.HEAVY,
            CombatJuiceTier.fromCombatEvent("full_charge", 1.0, null),
            "expected full_charge alias to infer HEAVY because fromWire(full_charge) maps to the same tier, actual tier differed"
        );
    }

    @Test
    void accept_null_event_returns_empty_command() {
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(null, 1_000L);

        assertNull(command.event(), "expected null combat event to produce an empty command because invalid input must be ignored safely");
        assertFalse(command.overlay().activeAt(1_000L), "expected null combat event to have no active overlay because no visual branch ran");
    }

    @Test
    void accept_clears_expired_overlay_before_next_command_snapshot() {
        CombatJuiceEvent overload = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.OVERLOAD,
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            1_000L
        );
        CombatJuiceSystem.LastCommand overloadCommand = CombatJuiceSystem.accept(overload, 1_000L);
        assertTrue(overloadCommand.overlay().activeAt(1_000L), "expected overload command to carry active overlay because overload creates a 10 tick vignette");

        CombatJuiceEvent hit = CombatJuiceEvent.hit(CombatSchool.BAOMAI, CombatJuiceTier.LIGHT, "attacker", "target", 0.0, 1.0, 1_501L);
        CombatJuiceSystem.LastCommand hitCommand = CombatJuiceSystem.accept(hit, 1_501L);

        assertFalse(hitCommand.overlay().activeAt(1_501L), "expected later hit command to drop expired overload overlay because command snapshots must not carry stale overlays");
        assertEquals(CombatJuiceEvent.Kind.HIT, hitCommand.event().kind(), "expected the post-overlay command to still process the new hit event, actual kind differed");
    }

    @Test
    void overload_red_freeze() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.OVERLOAD,
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "target",
            "",
            "",
            0.0,
            1.0,
            false,
            4_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 4_000L);

        assertEquals(10, HitStopController.remainingTicks("target", 4_000L));
        assertTrue(command.overlay().vignette());
        assertEquals(0x00FF2020, command.overlay().argb() & 0x00FFFFFF);
    }

    @Test
    void parry_pushback_both_sides() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.PARRY,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "defender",
            "",
            "",
            0.0,
            1.0,
            false,
            5_000L
        );
        ParryDodgeJuicePlanner.ParryPlan plan = ParryDodgeJuicePlanner.parry(event, false);

        assertEquals("attacker", plan.attackerPushback().entityUuid());
        assertEquals(-0.3, plan.attackerPushback().velocityZ(), 0.0001);
        assertEquals("defender", plan.defenderPushback().entityUuid());
        assertEquals(0.3, plan.defenderPushback().velocityZ(), 0.0001);
    }

    @Test
    void perfect_parry_white_flash() {
        CombatJuiceEvent event = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.PERFECT_PARRY,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "attacker",
            "defender",
            "",
            "",
            1.0,
            0.0,
            false,
            5_000L
        );
        CombatJuiceSystem.LastCommand command = CombatJuiceSystem.accept(event, 5_000L);

        assertNotNull(command.parry());
        assertTrue(command.parry().perfect());
        assertEquals(0x00FFFFFF, command.overlay().argb() & 0x00FFFFFF);
    }

    @Test
    void dodge_ghost_entity_fades() {
        ParryDodgeJuicePlanner.DodgeGhost ghost = ParryDodgeJuicePlanner.dodge("player", 0xFFCCAA88, 1_000L);

        assertEquals(0.4f, ghost.alphaAt(1_000L), 0.001f);
        assertTrue(ghost.alphaAt(1_250L) < ghost.alphaAt(1_000L));
        assertEquals(0.0f, ghost.alphaAt(1_500L), 0.001f);
    }

    @Test
    void fracture_tilts_limb() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("left_arm", "bone_fracture", 0.7f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertEquals(1, commands.size());
        assertEquals(5.0f, commands.get(0).limbTiltDegrees(), 0.001f);
    }

    @Test
    void severed_drip_particle() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("right_arm", "limb_severed", 0.9f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.get(0).dripParticle(), "expected explicit limb_severed wound to emit drip particles because only amputation-type wounds should look severed, actual command did not drip");
    }

    @Test
    void high_severity_cut_does_not_trigger_severed_visuals() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("right_arm", "cut", 0.95f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.isEmpty(), "expected high-severity non-amputation cut to avoid severed visuals because severity alone is not an amputation signal, actual commands=" + commands);
    }

    @Test
    void wound_visual_planner_handles_blank_network_fields() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound(null, null, 0.95f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L)),
            List.of(new StatusEffectStore.Effect(null, null, null, 1, 1_000L, 0, null, 0)),
            false
        );

        assertTrue(commands.isEmpty(), "expected blank wound/effect ids to be ignored because missing optional network fields must not create false visuals, actual commands=" + commands);
    }

    @Test
    void contamination_meridian_glow() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(
            List.of(new WoundsStore.Wound("chest", "qi_wound", 0.2f, WoundsStore.HealingState.BLEEDING, 0.8f, false, 0L)),
            List.of(),
            false
        );

        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::meridianGlow));
        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::coughAudio));
    }

    @Test
    void exhausted_stumble_interval() {
        List<WoundWorldVisualPlanner.WoundCommand> commands = WoundWorldVisualPlanner.plan(List.of(), List.of(), true);

        assertTrue(commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::exhaustedStumble));
    }

    @Test
    void exhausted_stumble_from_status_effect_snapshot() {
        // 虚脱已收敛到标准 debuff（id "exhausted"）；快照里存在该条目即触发踉跄。
        StatusEffectStore.Effect exhausted = new StatusEffectStore.Effect(
            "exhausted", "虚脱", StatusEffectStore.Kind.DEBUFF, 1, 4000L, 0xFFFF8030, "全力一击", 5);
        List<WoundWorldVisualPlanner.WoundCommand> commands =
            WoundWorldVisualPlanner.plan(List.of(), List.of(exhausted), false);

        assertTrue(
            commands.stream().anyMatch(WoundWorldVisualPlanner.WoundCommand::exhaustedStumble),
            "status_snapshot 含虚脱 debuff(id=exhausted) 时应产生踉跄命令");
    }

    @Test
    void no_exhausted_stumble_when_effect_expired_or_absent() {
        // 无虚脱条目 → 无踉跄。
        assertFalse(
            WoundWorldVisualPlanner.plan(List.of(), List.of(), false).stream()
                .anyMatch(WoundWorldVisualPlanner.WoundCommand::exhaustedStumble));
        // remaining_ms=0（过期）→ 无踉跄（status_snapshot 全量替换后理应不下发，双重保险）。
        StatusEffectStore.Effect expired = new StatusEffectStore.Effect(
            "exhausted", "虚脱", StatusEffectStore.Kind.DEBUFF, 1, 0L, 0xFFFF8030, "全力一击", 5);
        assertFalse(
            WoundWorldVisualPlanner.plan(List.of(), List.of(expired), false).stream()
                .anyMatch(WoundWorldVisualPlanner.WoundCommand::exhaustedStumble),
            "过期虚脱条目不应触发踉跄");
    }

    @Test
    void kill_slowmo_only_for_killer() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent remoteKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "someone_else",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertFalse(KillJuiceController.trigger(remoteKill, profile, 1_000L).activeAt(1_000L), "expected remote kill to suppress slowmo because local player is not attacker, actual state was active");

        CombatJuiceEvent unknownLocalKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertFalse(KillJuiceController.trigger(unknownLocalKill, profile, 1_000L).activeAt(1_000L), "expected blank local uuid to suppress kill slowmo because unknown identity must not count as local attacker, actual state was active");

        CombatJuiceEvent localKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "rat",
            0.0,
            1.0,
            false,
            1_000L
        );

        assertTrue(KillJuiceController.trigger(localKill, profile, 1_000L).activeAt(1_000L), "expected local attacker kill to trigger slowmo because local uuid matches attacker, actual state was inactive");
        assertTrue(KillJuiceController.fovDelta(1_000L) < 0.0, "expected local kill slowmo to push FOV negative because kill juice adds impact zoom, actual delta=" + KillJuiceController.fovDelta(1_000L));
    }

    @Test
    void rare_drop_golden_pillar() {
        CombatJuiceEvent localKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.ZHENFA,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "elite",
            0.0,
            1.0,
            true,
            1_000L
        );

        KillJuiceController.KillState state =
            KillJuiceController.trigger(localKill, CombatJuiceProfile.select(CombatSchool.ZHENFA, CombatJuiceTier.CRITICAL), 1_000L);

        assertTrue(state.rareDrop());
    }

    @Test
    void multi_kill_counter_stacks() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent kill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "target",
            0.0,
            1.0,
            false,
            1_000L
        );

        KillJuiceController.trigger(kill, profile, 1_000L);
        KillJuiceController.trigger(kill, profile, 4_000L);

        assertEquals(2, KillJuiceController.multiKill().count(), "expected second kill inside 5s window to stack multi-kill count to 2, actual count differed");
        assertEquals(1.2, KillJuiceController.multiKill().shakeMultiplier(), 0.0001, "expected second kill to raise shake multiplier to 1.2, actual multiplier differed");
        assertEquals(1.1, KillJuiceController.multiKill().pitchMultiplier(), 0.0001, "expected second kill to raise pitch multiplier to 1.1, actual multiplier differed");
    }

    @Test
    void multi_kill_counter_expires_after_window() {
        CombatJuiceProfile profile = CombatJuiceProfile.select(CombatSchool.BAOMAI, CombatJuiceTier.CRITICAL);
        CombatJuiceEvent kill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "attacker",
            "target",
            "attacker",
            "target",
            0.0,
            1.0,
            false,
            1_000L
        );

        KillJuiceController.trigger(kill, profile, 1_000L);
        KillJuiceController.trigger(kill, profile, 6_001L);

        assertEquals(1, KillJuiceController.multiKill().count(), "expected kill after the 5s window to reset multi-kill count because previous chain expired, actual count differed");
    }

    @Test
    void clearOnDisconnectResetsEveryOldSessionRuntimeEffectAndAllowsFreshReuse() {
        CombatJuiceEvent oldOverload = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.OVERLOAD,
            CombatSchool.BAOMAI,
            CombatJuiceTier.LIGHT,
            "old-attacker",
            "old-target",
            "",
            "",
            1.0,
            0.0,
            false,
            1_000L
        );
        CombatJuiceEvent oldCollision = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.QI_COLLISION,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.HEAVY,
            "old-attacker",
            "old-tinted-target",
            "",
            "",
            0.0,
            1.0,
            false,
            1_001L
        );
        CombatJuiceEvent oldParry = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.PERFECT_PARRY,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "old-attacker",
            "old-defender",
            "",
            "",
            0.0,
            1.0,
            false,
            1_002L
        );
        CombatJuiceEvent oldDodge = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.DODGE,
            CombatSchool.ZHENMAI,
            CombatJuiceTier.LIGHT,
            "",
            "old-dodger",
            "",
            "",
            0.0,
            1.0,
            false,
            1_003L
        );
        CombatJuiceEvent oldKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "old-attacker",
            "old-victim",
            "old-attacker",
            "old victim",
            0.0,
            1.0,
            false,
            1_004L
        );

        CombatJuiceSystem.accept(oldOverload, 1_000L);
        CombatJuiceSystem.accept(oldCollision, 1_001L);
        CombatJuiceSystem.accept(oldParry, 1_002L);
        CombatJuiceSystem.accept(oldDodge, 1_003L);
        CombatJuiceSystem.accept(oldKill, 1_004L);

        assertTrue(HitStopController.isFrozen("old-target", 1_004L), "前置：旧 session overload 必须留下 freeze");
        assertNotEquals(CameraShakeController.ZERO, CameraShakeController.activeOffsets(1_004L),
            "前置：旧 session overload 必须留下 shake");
        assertEquals(CameraShakeController.Source.HIT, CameraShakeController.activeSource(),
            "前置：旧 session shake 必须登记 owner");
        assertTrue(EntityTintController.activeTint("old-tinted-target", 1_004L).activeAt(1_004L),
            "前置：旧 session collision 必须留下 tint");
        assertTrue(CombatJuiceSystem.activeOverlay(1_004L).activeAt(1_004L),
            "前置：旧 session parry 必须留下 overlay");
        assertNotNull(CombatJuiceSystem.lastParry(), "前置：旧 session parry 必须留下 parry plan");
        assertNotNull(CombatJuiceSystem.lastGhost(), "前置：旧 session dodge 必须留下 ghost");
        assertTrue(KillJuiceController.activeKill(1_004L).activeAt(1_004L),
            "前置：旧 session local kill 必须留下 slowmo");
        assertEquals(1, KillJuiceController.multiKill().count(), "前置：旧 session kill 必须起连杀计数");

        CombatJuiceSystem.clearOnDisconnect();

        assertEquals(CombatJuiceSystem.LastCommand.empty(), CombatJuiceSystem.lastCommand(),
            "断线必须清空 lastCommand，不能向新 session 暴露旧指令");
        assertEquals(CombatJuiceSystem.Overlay.none(), CombatJuiceSystem.activeOverlay(1_004L),
            "断线必须清空 active overlay");
        assertNull(CombatJuiceSystem.lastParry(), "断线必须清空旧 parry plan");
        assertNull(CombatJuiceSystem.lastGhost(), "断线必须清空旧 dodge ghost");
        assertFalse(HitStopController.isFrozen("old-target", 1_004L), "断线必须清空旧 freeze");
        assertEquals(CameraShakeController.ZERO, CameraShakeController.activeOffsets(1_004L),
            "断线必须清空旧 camera shake");
        assertEquals(CameraShakeController.Source.NONE, CameraShakeController.activeSource(),
            "断线必须清空 shake owner");
        assertEquals(EntityTintController.Tint.none(), EntityTintController.activeTint("old-tinted-target", 1_004L),
            "断线必须清空旧实体 tint cache");
        assertEquals(KillJuiceController.KillState.none(), KillJuiceController.activeKill(1_004L),
            "断线必须清空旧 kill slowmo");
        assertEquals(KillJuiceController.MultiKillState.empty(), KillJuiceController.multiKill(),
            "断线必须清空旧 multi-kill state");

        CombatJuiceSystem.clearOnDisconnect();
        assertEquals(CombatJuiceSystem.LastCommand.empty(), CombatJuiceSystem.lastCommand(),
            "重复断线清理必须保持 neutral lastCommand");
        assertEquals(CameraShakeController.Source.NONE, CameraShakeController.activeSource(),
            "重复断线清理必须保持无 shake owner");
        assertEquals(KillJuiceController.MultiKillState.empty(), KillJuiceController.multiKill(),
            "重复断线清理必须保持 empty multi-kill");

        CombatJuiceEvent freshHit = CombatJuiceEvent.hit(
            CombatSchool.ZHENFA, CombatJuiceTier.HEAVY, "fresh-attacker", "fresh-target", 1.0, 0.0, 2_000L
        );
        CombatJuiceEvent freshCollision = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.QI_COLLISION,
            CombatSchool.ZHENFA,
            CombatJuiceTier.HEAVY,
            "fresh-attacker",
            "fresh-tinted-target",
            "",
            "",
            0.0,
            1.0,
            false,
            2_001L
        );
        CombatJuiceEvent freshKill = new CombatJuiceEvent(
            CombatJuiceEvent.Kind.KILL,
            CombatSchool.BAOMAI,
            CombatJuiceTier.CRITICAL,
            "fresh-attacker",
            "fresh-victim",
            "fresh-attacker",
            "fresh victim",
            0.0,
            1.0,
            false,
            2_002L
        );
        CombatJuiceSystem.LastCommand freshCommand = CombatJuiceSystem.accept(freshHit, 2_000L);
        CombatJuiceSystem.accept(freshCollision, 2_001L);
        CombatJuiceSystem.accept(freshKill, 2_002L);
        assertEquals(freshHit, freshCommand.event(), "fresh session 的新命中必须仍可写入 command");
        assertTrue(HitStopController.isFrozen("fresh-target", 2_002L), "fresh session 必须可重新创建 freeze");
        assertNotEquals(CameraShakeController.ZERO, CameraShakeController.activeOffsets(2_002L),
            "fresh session 必须可重新创建 shake");
        assertEquals(CameraShakeController.Source.HIT, CameraShakeController.activeSource(),
            "fresh session 的 shake 必须重新登记 owner");
        assertTrue(EntityTintController.activeTint("fresh-tinted-target", 2_002L).activeAt(2_002L),
            "fresh session 必须可重新创建 tint cache");
        assertTrue(KillJuiceController.activeKill(2_002L).activeAt(2_002L),
            "fresh session 必须可重新创建 kill slowmo");
        assertEquals(1, KillJuiceController.multiKill().count(),
            "fresh session 的首杀必须从 empty multi-kill 重新起算");
    }

    @Test
    void clearOnDisconnectPreservesBootstrapAndTickWiring() {
        CombatJuiceSystem.bootstrap();
        AtomicBoolean bootstrapped = bootstrappedFlag();
        assertTrue(bootstrapped.get(), "前置：生产 bootstrap 必须已登记一次 tick listener");

        CombatJuiceSystem.clearOnDisconnect();

        assertTrue(bootstrapped.get(),
            "断线清理只能清 session data，不能复位 BOOTSTRAPPED 或摘除 production tick wiring");

        CombatJuiceEvent hit = CombatJuiceEvent.hit(
            CombatSchool.GENERIC, CombatJuiceTier.LIGHT, "fresh-attacker", "fresh-target", 0.0, 1.0, 3_000L
        );
        CombatJuiceSystem.accept(hit, 3_000L);
        CombatJuiceSystem.tick(3_050L);

        assertEquals(1, HitStopController.remainingTicks("fresh-target", 3_050L),
            "断线后既有 tick wiring 必须继续推进 fresh session freeze，不得被 teardown 摘除");
    }

    private static AtomicBoolean bootstrappedFlag() {
        try {
            Field field = CombatJuiceSystem.class.getDeclaredField("BOOTSTRAPPED");
            field.setAccessible(true);
            return (AtomicBoolean) field.get(null);
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法读取 CombatJuiceSystem.BOOTSTRAPPED 来核验断线不会重置 production wiring", exception);
        }
    }

    @Test
    void pvp_calibration_matrix_covers_49_pairings() {
        List<CombatJuiceCalibration.PvpPairing> pairings = CombatJuiceCalibration.pvpPairings();
        assertEquals(49, pairings.size());
        assertFalse(pairings.stream().anyMatch(CombatJuiceCalibration.PvpPairing::inputLagRisk));
        assertTrue(pairings.stream().anyMatch(CombatJuiceCalibration.PvpPairing::sameQiColor));
    }

    @Test
    void mixed_battle_budget_stays_above_30fps_floor() {
        CombatJuiceCalibration.PerformanceBudget budget = CombatJuiceCalibration.mixedBattleBudget(10, 10);

        assertEquals(40, budget.maxConcurrentJuiceEvents(), "expected 5v5 to budget 40 concurrent juice events because budget is 4 events per player across 10 players, actual event budget differed");
        assertTrue(budget.passesPlanFloor(), "expected 5v5 10min budget to satisfy 30fps floor because plan requires that scenario, actual budget=" + budget);
    }

    @Test
    void mixed_battle_budget_clamps_large_inputs_without_overflow() {
        CombatJuiceCalibration.PerformanceBudget budget = CombatJuiceCalibration.mixedBattleBudget(Integer.MAX_VALUE, 10);

        assertEquals(Integer.MAX_VALUE, budget.maxConcurrentJuiceEvents(), "expected huge player count to clamp maxConcurrentJuiceEvents to Integer.MAX_VALUE because int budget field cannot represent larger values, actual budget differed");
        assertEquals(30, budget.estimatedFpsFloor(), "expected huge event count to clamp estimated FPS floor to 30 instead of overflowing below the minimum, actual floor differed");
    }
}
