package com.bong.client.combat.store;

import com.bong.client.death.DeathCinematicState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DisconnectClearStoreTest {
    @AfterEach
    void tearDown() {
        AscensionQuotaStore.resetForTests();
        CarrierStateStore.resetForTests();
        DamageFloaterStore.resetForTests();
        DeathStateStore.resetForTests();
        DerivedAttrsStore.resetForTests();
        DuguPoisonStateStore.resetForTests();
        FullPowerStateStore.resetForTests();
        TerminateStateStore.resetForTests();
        VortexStateStore.resetForTests();
    }

    @Test
    void ascensionQuotaClearPublishesEmptyAndKeepsListenerForNewSession() {
        List<AscensionQuotaStore.State> notified = new ArrayList<>();
        AscensionQuotaStore.addListener(notified::add);
        AscensionQuotaStore.State oldState = new AscensionQuotaStore.State(3, 8, 5, 64.0, 2.0, "old-session");
        AscensionQuotaStore.replace(oldState);

        AscensionQuotaStore.clearOnDisconnect();

        assertEquals(AscensionQuotaStore.State.EMPTY, AscensionQuotaStore.snapshot(),
            "断线必须发布完整空配额态，不能保留上一会话的槽位或真元统计");
        assertEquals(List.of(oldState, AscensionQuotaStore.State.EMPTY), notified,
            "clearOnDisconnect 必须经生产 replace 路径通知已注册 listener");
        AscensionQuotaStore.State newState = new AscensionQuotaStore.State(1, 4, 3, 32.0, 1.0, "new-session");
        AscensionQuotaStore.replace(newState);
        assertEquals(newState, AscensionQuotaStore.snapshot(), "新会话 quota payload 必须仍可写入");
        assertEquals(List.of(oldState, AscensionQuotaStore.State.EMPTY, newState), notified,
            "clearOnDisconnect 不得移除 listener，后续新会话写入仍必须通知");
    }

    @Test
    void carrierClearResetsExactNoneAndAllowsNewSessionWrite() {
        CarrierStateStore.State oldState = new CarrierStateStore.State(
            CarrierStateStore.Phase.CHARGED, 1f, 18f, 20f, 300L, 42L);
        CarrierStateStore.replace(oldState);

        CarrierStateStore.clearOnDisconnect();

        assertEquals(CarrierStateStore.State.NONE, CarrierStateStore.snapshot(),
            "断线必须清空 carrier phase、进度、封存真元和实例 ID");
        CarrierStateStore.State newState = new CarrierStateStore.State(
            CarrierStateStore.Phase.CHARGING, 0.25f, 4f, 16f, 80L, 77L);
        CarrierStateStore.replace(newState);
        assertEquals(newState, CarrierStateStore.snapshot(), "新会话 carrier payload 必须仍可写入");
    }

    @Test
    void damageFloaterClearRemovesAllEntriesAndAllowsNewSessionFloater() {
        DamageFloaterStore.publish(floater("12", 1_000L));
        DamageFloaterStore.publish(floater("暴击", 1_010L));
        assertEquals(2, DamageFloaterStore.snapshot(1_020L).size(), "测试前置：旧会话浮字必须存在");

        DamageFloaterStore.clearOnDisconnect();

        assertTrue(DamageFloaterStore.snapshot(1_020L).isEmpty(),
            "断线必须清空全部旧会话伤害浮字，不能等自然过期");
        DamageFloaterStore.Floater newFloater = floater("新局", 2_000L);
        DamageFloaterStore.publish(newFloater);
        assertEquals(List.of(newFloater), DamageFloaterStore.snapshot(2_000L),
            "清理后新会话浮字必须仍可写入并显示");
    }

    @Test
    void deathClearRestoresCompleteHiddenStateAndAllowsNewSessionWrite() {
        DeathStateStore.State oldState = new DeathStateStore.State(
            true, "tribulation", 0.75f, List.of("旧遗念"), 99_999L, true, true,
            "fortune", 4, "negative", 72.0, 100, 28.0, 12, 1.5, true,
            activeCinematic());
        DeathStateStore.replace(oldState);

        DeathStateStore.clearOnDisconnect();

        assertEquals(DeathStateStore.State.HIDDEN, DeathStateStore.snapshot(),
            "断线必须恢复完整 HIDDEN 初始态，而非只把 visible 设为 false");
        DeathStateStore.State newState = new DeathStateStore.State(
            true, "pk", 0.5f, List.of("新遗念"), 12_345L, false, true);
        DeathStateStore.replace(newState);
        assertEquals(newState, DeathStateStore.snapshot(), "新会话 death_screen 必须仍可写入");
    }

    @Test
    void derivedAttrsClearResetsExactNoneAndAllowsNewSessionWrite() {
        DerivedAttrsStore.State oldState = new DerivedAttrsStore.State(
            true, 0.8f, 99_999L, true, 88_888L, true, "striking", 0.9f, 3, true);
        DerivedAttrsStore.replace(oldState);

        DerivedAttrsStore.clearOnDisconnect();

        assertEquals(DerivedAttrsStore.State.NONE, DerivedAttrsStore.snapshot(),
            "断线必须清空飞行、相位、劫锁、伪皮和涡流派生属性");
        DerivedAttrsStore.State newState = new DerivedAttrsStore.State(
            false, 0f, 0L, true, 2_000L, false, "", 0.1f, 1, false);
        DerivedAttrsStore.replace(newState);
        assertEquals(newState, DerivedAttrsStore.snapshot(), "新会话 derived attrs 必须仍可写入");
    }

    @Test
    void duguPoisonClearResetsExactNoneAndAllowsNewSessionWrite() {
        DuguPoisonStateStore.State oldState = new DuguPoisonStateStore.State(
            true, "lung", "旧施毒者", 500L, 4, 1.5, 0.2, 30.0, 600L);
        DuguPoisonStateStore.replace(oldState);

        DuguPoisonStateStore.clearOnDisconnect();

        assertEquals(DuguPoisonStateStore.State.NONE, DuguPoisonStateStore.snapshot(),
            "断线必须清空毒蛊附着、施毒者、经脉损失和 server tick");
        DuguPoisonStateStore.State newState = new DuguPoisonStateStore.State(
            true, "heart", "新施毒者", 700L, 2, 0.5, 0.7, 45.0, 800L);
        DuguPoisonStateStore.replace(newState);
        assertEquals(newState, DuguPoisonStateStore.snapshot(), "新会话毒蛊 payload 必须仍可写入");
    }

    @Test
    void fullPowerClearResetsChargingExhaustedAndLastReleaseAndAllowsNewSessionWrites() {
        FullPowerStateStore.updateCharging(new FullPowerStateStore.ChargingState(
            true, "old-caster", 60.0, 80.0, 100L, 1_000L));
        FullPowerStateStore.updateExhausted(new FullPowerStateStore.ExhaustedState(
            true, "old-caster", 100L, 500L, 1_000L));
        FullPowerStateStore.recordRelease(new FullPowerStateStore.ReleaseEvent(
            "old-caster", "old-target", 80.0, 500L, 1_100L));

        FullPowerStateStore.clearOnDisconnect();

        assertEquals(FullPowerStateStore.ChargingState.inactive(), FullPowerStateStore.charging(),
            "断线必须清空 charging，不能残留旧会话蓄力条");
        assertEquals(FullPowerStateStore.ExhaustedState.inactive(), FullPowerStateStore.exhausted(),
            "断线必须清空 exhausted，不能残留旧会话虚脱态");
        assertEquals(FullPowerStateStore.ReleaseEvent.empty(), FullPowerStateStore.lastRelease(),
            "断线必须清空 lastRelease，不能泄漏旧会话命中反馈");
        FullPowerStateStore.ChargingState newCharging = new FullPowerStateStore.ChargingState(
            true, "new-caster", 10.0, 40.0, 700L, 2_000L);
        FullPowerStateStore.ExhaustedState newExhausted = new FullPowerStateStore.ExhaustedState(
            true, "new-caster", 700L, 900L, 2_000L);
        FullPowerStateStore.ReleaseEvent newRelease = new FullPowerStateStore.ReleaseEvent(
            "new-caster", "new-target", 40.0, 900L, 2_100L);
        FullPowerStateStore.updateCharging(newCharging);
        FullPowerStateStore.updateExhausted(newExhausted);
        FullPowerStateStore.recordRelease(newRelease);
        assertEquals(newCharging, FullPowerStateStore.charging(), "新会话 charging 必须仍可写入");
        assertEquals(newExhausted, FullPowerStateStore.exhausted(), "新会话 exhausted 必须仍可写入");
        assertEquals(newRelease, FullPowerStateStore.lastRelease(), "新会话 release 必须仍可写入");
    }

    @Test
    void terminateClearRestoresCompleteHiddenStateAndAllowsNewSessionWrite() {
        TerminateStateStore.State oldState = new TerminateStateStore.State(
            true, "旧遗言", "旧结局", "旧转世建议");
        TerminateStateStore.replace(oldState);

        TerminateStateStore.clearOnDisconnect();

        assertEquals(TerminateStateStore.State.HIDDEN, TerminateStateStore.snapshot(),
            "断线必须恢复完整 HIDDEN 初始态，而非只隐藏终结屏");
        TerminateStateStore.State newState = new TerminateStateStore.State(
            true, "新遗言", "新结局", "新转世建议");
        TerminateStateStore.replace(newState);
        assertEquals(newState, TerminateStateStore.snapshot(), "新会话 termination payload 必须仍可写入");
    }

    @Test
    void vortexClearResetsExactNoneAndAllowsNewSessionWrite() {
        VortexStateStore.State oldState = new VortexStateStore.State(
            true, 12f, 3f, 80f, 200L, 4, "woliu", 0.8f, 90_000L, "major", 16f, 0.7f, 91_000L);
        VortexStateStore.replace(oldState);

        VortexStateStore.clearOnDisconnect();

        assertEquals(VortexStateStore.State.NONE, VortexStateStore.snapshot(),
            "断线必须清空涡流、冷却、反噬和紊流残留");
        VortexStateStore.State newState = new VortexStateStore.State(
            true, 6f, 1f, 40f, 100L, 1, "woliu_new", 0.2f, 2_000L, "", 0f, 0f, 0L);
        VortexStateStore.replace(newState);
        assertEquals(newState, VortexStateStore.snapshot(), "新会话 vortex payload 必须仍可写入");
    }

    private static DamageFloaterStore.Floater floater(String text, long createdAtMs) {
        return new DamageFloaterStore.Floater(
            1.0, 2.0, 3.0, text, 0xFF00FF00, DamageFloaterStore.Kind.HIT, createdAtMs);
    }

    private static DeathCinematicState activeCinematic() {
        return new DeathCinematicState(
            true, "old-character", DeathCinematicState.Phase.DARKNESS, 10L, 40L, 100L, 400L,
            new DeathCinematicState.Roll(0.5, 0.4, 0.6, DeathCinematicState.RollResult.FALL),
            List.of("旧幻境"), true, 4, "negative", true, 80L, false, 5_000L);
    }
}
