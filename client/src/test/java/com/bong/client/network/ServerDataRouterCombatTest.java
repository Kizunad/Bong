package com.bong.client.network;

import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Integration: verify the default ServerDataRouter routes combat UI payloads
 * to the right handlers (plan §U1–U7 wiring check).
 */
class ServerDataRouterCombatTest {

    @BeforeEach
    @AfterEach
    void resetState() {
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
        assertFalse(r.isParseError(), "expected combat_event JSON to parse because payload is valid, actual parse error=" + r.logMessage());
        assertTrue(r.isHandled(), "expected combat_event to be handled because it has one valid hit event, actual message=" + r.logMessage());
        assertEquals(1, DamageFloaterStore.snapshot(System.currentTimeMillis()).size(), "expected one damage floater because one valid hit event was routed, actual floater count differed");
        CombatJuiceSystem.LastCommand last = CombatJuiceSystem.lastCommand();
        assertNotNull(last, "expected non-null combat juice command because combat_event(hit) was routed");
        assertNotNull(last.event(), "expected non-null combat juice event because combat_event(hit) should enter CombatJuiceSystem");
        assertEquals(
            "target",
            last.event().targetUuid(),
            "expected targetUuid to be 'target' because payload contains target_uuid=target, actual targetUuid=" + last.event().targetUuid()
        );
    }

    @Test void dropsBlankCombatEventTextWithoutPublishingFloater() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = """
            {"v":1,"type":"combat_event","events":[
              {"amount":0,"text":"   ","target_uuid":"target"}
            ]}""";

        ServerDataRouter.RouteResult r = router.route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(r.isParseError(), "expected blank-text combat_event JSON to parse because syntax is valid, actual parse error=" + r.logMessage());
        assertFalse(r.isHandled(), "expected blank combat event to be ignored because it has no text, amount, or kind fallback, actual message=" + r.logMessage());
        assertEquals(0, DamageFloaterStore.snapshot(System.currentTimeMillis()).size(), "expected no floater because blank combat event should be discarded, actual floater count differed");
        assertNull(CombatJuiceSystem.lastCommand().event(), "expected no combat juice command because invalid blank combat event should not be accepted");
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
