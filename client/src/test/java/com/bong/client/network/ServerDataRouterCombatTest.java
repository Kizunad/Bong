package com.bong.client.network;

import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Integration: verify the default ServerDataRouter routes combat UI payloads
 * to the right handlers (plan §U1–U7 wiring check).
 */
class ServerDataRouterCombatTest {

    @AfterEach void tearDown() {
        DamageFloaterStore.resetForTests();
        CombatJuiceSystem.resetForTests();
        StatusEffectStore.resetForTests();
        WoundsStore.resetForTests();
    }

    @Test void registersCombatPayloadTypes() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        assertTrue(router.registeredTypes().contains("combat_event"));
        assertTrue(router.registeredTypes().contains("status_snapshot"));
        assertTrue(router.registeredTypes().contains("derived_attrs_sync"));
        assertTrue(router.registeredTypes().contains("death_screen"));
        assertTrue(router.registeredTypes().contains("terminate_screen"));
        assertTrue(router.registeredTypes().contains("wounds_snapshot"));
        assertTrue(router.registeredTypes().contains("tribulation_broadcast"));
    }

    @Test void routesCombatEventEndToEnd() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = """
            {"v":1,"type":"combat_event","events":[
              {"kind":"hit","amount":10,"color":-65536,"target_uuid":"target"}
            ]}""";
        ServerDataRouter.RouteResult r = router.route(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertFalse(r.isParseError());
        assertTrue(r.isHandled());
        assertEquals(1, DamageFloaterStore.snapshot(System.currentTimeMillis()).size());
        assertEquals("target", CombatJuiceSystem.lastCommand().event().targetUuid());
    }

    @Test void routesStatusSnapshotEndToEnd() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = """
            {"v":1,"type":"status_snapshot","effects":[
              {"id":"burn","name":"灼烧","kind":"dot","stacks":1,"remaining_ms":3000}
            ]}""";
        ServerDataRouter.RouteResult r = router.route(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isHandled());
        assertEquals(1, StatusEffectStore.snapshot().size());
    }
}
