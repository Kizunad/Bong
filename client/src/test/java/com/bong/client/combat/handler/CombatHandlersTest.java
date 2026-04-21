package com.bong.client.combat.handler;

import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.combat.store.TribulationBroadcastStore;
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
        TribulationBroadcastStore.resetForTests();
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
             "actor_name":"甲","stage":"warn","world_x":10,"world_z":-5,
             "expires_at_ms":9999999999}""";
        new TribulationBroadcastHandler().handle(parse(json));
        assertTrue(TribulationBroadcastStore.snapshot().active());
        assertEquals("warn", TribulationBroadcastStore.snapshot().stage());
    }

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
