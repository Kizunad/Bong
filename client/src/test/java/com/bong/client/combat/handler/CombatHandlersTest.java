package com.bong.client.combat.handler;

import com.bong.client.combat.store.AscensionQuotaStore;
import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.combat.store.TribulationBroadcastStore;
import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.combat.store.VortexStateStore;
import com.bong.client.combat.store.WoundsStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

class CombatHandlersTest {

    @AfterEach
    void tearDown() {
        DamageFloaterStore.resetForTests();
        StatusEffectStore.resetForTests();
        DerivedAttrsStore.resetForTests();
        DeathStateStore.resetForTests();
        TerminateStateStore.resetForTests();
        WoundsStore.resetForTests();
        TribulationStateStore.resetForTests();
        TribulationBroadcastStore.resetForTests();
        AscensionQuotaStore.resetForTests();
        VortexStateStore.resetForTests();
    }

    @Test
    void combatEventHandlerAcceptsEvents() {
        String json = """
            {"v":1,"type":"combat_event","events":[
              {"kind":"hit","amount":12,"x":1.0,"y":2.0,"z":3.0,"color":-65536},
              {"kind":"crit","text":"25"}
            ]}""";
        ServerDataDispatch dispatch = new CombatEventHandler().handle(parse(json));
        assertTrue(dispatch.handled());
        assertEquals(2, DamageFloaterStore.snapshot(System.currentTimeMillis()).size());
    }

    @Test
    void combatEventHandlerRejectsWhenNoArray() {
        ServerDataDispatch dispatch = new CombatEventHandler().handle(parse(
            "{\"v\":1,\"type\":\"combat_event\"}"
        ));
        assertFalse(dispatch.handled());
    }

    @Test
    void statusSnapshotPopulatesStore() {
        String json = """
            {"v":1,"type":"status_snapshot","effects":[
              {"id":"burn","name":"灼烧","kind":"dot","stacks":2,"remaining_ms":4000,
               "source_color":-65536,"source_label":"zombie","dispel":3}
            ]}""";
        ServerDataDispatch dispatch = new StatusSnapshotHandler().handle(parse(json));
        assertTrue(dispatch.handled());
        assertEquals(1, StatusEffectStore.snapshot().size());
        assertEquals("burn", StatusEffectStore.snapshot().get(0).id());
    }

    @Test
    void derivedAttrsSyncRoundTrips() {
        String json = """
            {"v":1,"type":"derived_attrs_sync","flying":true,"flying_qi_remaining":0.4,
             "tribulation_locked":true,"tribulation_stage":"warn"}""";
        ServerDataDispatch dispatch = new DerivedAttrsHandler().handle(parse(json));
        assertTrue(dispatch.handled());
        DerivedAttrsStore.State s = DerivedAttrsStore.snapshot();
        assertTrue(s.flying());
        assertTrue(s.tribulationLocked());
        assertEquals(0.4f, s.flyingQiRemaining(), 1e-5);
    }

    @Test
    void vortexStatePopulatesStore() {
        String json = """
            {"v":1,"type":"vortex_state","caster":"player:test","active":true,
             "center":[1,64,2],"radius":1.5,"delta":0.25,"env_qi_at_cast":0.9,
             "maintain_remaining_ticks":80,"intercepted_count":3,
             "active_skill_id":"woliu.heart","charge_progress":0.5,
             "cooldown_until_ms":9000,"backfire_level":"micro_tear",
             "turbulence_radius":12,"turbulence_intensity":0.75,
             "turbulence_until_ms":12000}""";
        ServerDataDispatch dispatch = new VortexStateHandler().handle(parse(json));
        assertTrue(dispatch.handled());
        VortexStateStore.State state = VortexStateStore.snapshot();
        assertTrue(state.active());
        assertEquals(1.5f, state.radius(), 1e-5);
        assertEquals(0.25f, state.delta(), 1e-5);
        assertEquals(80L, state.maintainRemainingTicks());
        assertEquals(3, state.interceptedCount());
        assertEquals("woliu.heart", state.activeSkillId());
        assertEquals(0.5f, state.chargeProgress(), 1e-5);
        assertEquals(9000L, state.cooldownUntilMs());
        assertEquals("micro_tear", state.backfireLevel());
        assertEquals(12f, state.turbulenceRadius(), 1e-5);
        assertEquals(0.75f, state.turbulenceIntensity(), 1e-5);
        assertEquals(12000L, state.turbulenceUntilMs());
    }

    @Test
    void deathScreenHandlerVisibleFalseHides() {
        // First set visible, then hide.
        new DeathScreenHandler().handle(parse(
            "{\"v\":1,\"type\":\"death_screen\",\"visible\":true,\"cause\":\"pk\"}"));
        assertTrue(DeathStateStore.snapshot().visible());
        new DeathScreenHandler().handle(parse(
            "{\"v\":1,\"type\":\"death_screen\",\"visible\":false}"));
        assertFalse(DeathStateStore.snapshot().visible());
    }

    @Test
    void deathScreenHandlerPopulatesLifecycleNoticeFields() {
        String json = """
            {"v":1,"type":"death_screen","visible":true,"cause":"negative_zone",
             "luck_remaining":0.65,"final_words":["劫未尽"],"countdown_until_ms":12345,
             "can_reincarnate":true,"can_terminate":true,"stage":"tribulation",
             "death_number":4,"zone_kind":"negative",
             "lifespan":{"years_lived":78.5,"cap_by_realm":80,"remaining_years":1.5,
                         "death_penalty_years":4,"tick_rate_multiplier":2.0,"is_wind_candle":true},
             "cinematic":{"v":1,"character_id":"offline:Azure","phase":"roll",
                          "phase_tick":0,"phase_duration_ticks":80,
                          "total_elapsed_ticks":80,"total_duration_ticks":380,
                          "roll":{"probability":0.65,"threshold":0.65,"luck_value":0.42,"result":"pending"},
                          "insight_text":["坍缩渊，概不赊欠。"],"is_final":false,
                          "death_number":4,"zone_kind":"negative","tsy_death":true,
                          "rebirth_weakened_ticks":3600,"skip_predeath":false}}
            """;

        new DeathScreenHandler().handle(parse(json));

        DeathStateStore.State s = DeathStateStore.snapshot();
        assertTrue(s.visible());
        assertEquals("tribulation", s.stage());
        assertEquals(4, s.deathNumber());
        assertEquals("negative", s.zoneKind());
        assertEquals(78.5, s.yearsLived(), 1e-9);
        assertEquals(80, s.lifespanCapByRealm());
        assertEquals(2.0, s.lifespanTickRateMultiplier(), 1e-9);
        assertTrue(s.windCandle());
        assertTrue(s.cinematic().active());
        assertEquals("offline:Azure", s.cinematic().characterId());
        assertEquals("negative", s.cinematic().zoneKind());
        assertTrue(s.cinematic().tsyDeath());
    }

    @Test
    void terminateScreenHandlerPopulatesFields() {
        String json = """
            {"v":1,"type":"terminate_screen","visible":true,
             "final_words":"终焉","epilogue":"归去","archetype_suggestion":"游侠"}""";
        new TerminateScreenHandler().handle(parse(json));
        TerminateStateStore.State s = TerminateStateStore.snapshot();
        assertTrue(s.visible());
        assertEquals("终焉", s.finalWords());
        assertEquals("游侠", s.archetypeSuggestion());
    }

    @Test
    void woundsSnapshotPopulatesStoreAndMirrors() {
        String json = """
            {"v":1,"type":"wounds_snapshot","wounds":[
              {"part":"chest","kind":"cut","severity":0.6,"state":"bleeding","infection":0.1}
            ]}""";
        ServerDataDispatch d = new WoundsSnapshotHandler().handle(parse(json));
        assertTrue(d.handled());
        assertEquals(1, WoundsStore.snapshot().size());
        assertTrue(WoundsStore.hasBleedingAny());
    }

    @Test
    void tribulationBroadcastActivation() {
        String json = """
            {"v":1,"type":"tribulation_broadcast","active":true,
             "actor_name":"甲","stage":"locked","world_x":10,"world_z":-5,
             "expires_at_ms":9999999999}""";
        new TribulationBroadcastHandler().handle(parse(json));
        assertTrue(TribulationBroadcastStore.snapshot().active());
        assertEquals("locked", TribulationBroadcastStore.snapshot().stage());
    }

    @Test
    void tribulationStatePopulatesStoreAndKeepsResultOnClear() {
        String activeJson = """
            {"v":1,"type":"tribulation_state","active":true,
             "char_id":"offline:Azure","actor_name":"Azure","kind":"du_xu","phase":"wave",
             "world_x":12.0,"world_z":-34.0,"wave_current":2,"wave_total":5,
             "started_tick":120,"phase_started_tick":2400,"next_wave_tick":2700,
             "failed":false,"half_step_on_success":true,
             "participants":["offline:Azure","offline:Beryl"],"result":null} """;
        ServerDataDispatch dispatch = new TribulationStateHandler().handle(parse(activeJson));
        assertTrue(dispatch.handled());
        TribulationStateStore.State active = TribulationStateStore.snapshot();
        assertTrue(active.active());
        assertEquals("offline:Azure", active.charId());
        assertEquals("Azure", active.actorName());
        assertEquals("wave", active.phase());
        assertEquals(2, active.waveCurrent());
        assertEquals(5, active.waveTotal());
        assertTrue(active.halfStepOnSuccess());
        assertEquals(2, active.participants().size());

        String clearJson = """
            {"v":1,"type":"tribulation_state","active":false,
             "char_id":"offline:Azure","actor_name":"Azure","kind":"du_xu","phase":"settle",
             "world_x":0,"world_z":0,"wave_current":5,"wave_total":0,
             "started_tick":0,"phase_started_tick":0,"next_wave_tick":0,
             "failed":false,"half_step_on_success":false,
             "participants":[],"result":"ascended"} """;
        new TribulationStateHandler().handle(parse(clearJson));
        TribulationStateStore.State cleared = TribulationStateStore.snapshot();
        assertFalse(cleared.active());
        assertEquals("settle", cleared.phase());
        assertEquals("ascended", cleared.result());
        assertEquals(5, cleared.waveCurrent());
    }

    @Test
    void ascensionQuotaPopulatesStore() {
        String json = """
            {"v":1,"type":"ascension_quota",
             "occupied_slots":1,"quota_limit":2,"available_slots":1,
             "total_world_qi":100.0,"quota_k":50.0,
             "quota_basis":"world_qi_budget.current_total"}""";
        ServerDataDispatch dispatch = new AscensionQuotaHandler().handle(parse(json));
        assertTrue(dispatch.handled());
        AscensionQuotaStore.State state = AscensionQuotaStore.snapshot();
        assertEquals(1, state.occupiedSlots());
        assertEquals(2, state.quotaLimit());
        assertEquals(1, state.availableSlots());
        assertEquals(100.0, state.totalWorldQi());
        assertEquals(50.0, state.quotaK());
        assertEquals("world_qi_budget.current_total", state.quotaBasis());
    }

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
