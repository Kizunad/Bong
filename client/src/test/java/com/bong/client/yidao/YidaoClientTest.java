package com.bong.client.yidao;

import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class YidaoClientTest {
    @BeforeEach
    void resetStores() {
        YidaoHudStateStore.resetForTests();
        YidaoNpcAiStateStore.resetForTests();
    }

    @Test
    void yidao_hud_state_payload_updates_store() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        var result = router.route("""
            {"v":1,"type":"yidao_hud_state","healer_id":"npc:doctor","reputation":7,"peace_mastery":48.0,"karma":3.5,"active_skill":"life_extension","patient_ids":["offline:Patient"],"patient_hp_percent":0.5,"patient_contam_total":1.25,"severed_meridian_count":1,"contract_count":2,"mass_preview_count":0}
            """, 256);

        assertTrue(result.isHandled());
        YidaoHudStateStore.Snapshot snapshot = YidaoHudStateStore.snapshot();
        assertEquals("npc:doctor", snapshot.healerId());
        assertEquals("life_extension", snapshot.activeSkill());
        assertEquals(1, snapshot.patientIds().size());
        assertEquals(0.5f, snapshot.patientHpPercent(), 0.0001f);
        assertEquals(2, snapshot.contractCount());
    }

    @Test
    void healer_npc_ai_state_payload_updates_store() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        var result = router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor","active_action":"meridian_repair","queue_len":3,"reputation":9,"retreating":false}
            """, 160);

        assertTrue(result.isHandled());
        YidaoNpcAiStateStore.Snapshot snapshot = YidaoNpcAiStateStore.snapshot("npc:doctor");
        assertEquals("npc:doctor", snapshot.healerId());
        assertEquals("meridian_repair", snapshot.activeAction());
        assertEquals(3, snapshot.queueLen());
        assertEquals(1, YidaoNpcAiStateStore.activeCount());
    }

    @Test
    void healer_npc_ai_state_payload_preserves_multiple_healers() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor-a","active_action":"meridian_repair","queue_len":3,"reputation":9,"retreating":false}
            """, 160);
        router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor-b","active_action":"life_extension","queue_len":1,"reputation":4,"retreating":false}
            """, 160);

        assertEquals(2, YidaoNpcAiStateStore.activeCount());
        assertEquals("meridian_repair", YidaoNpcAiStateStore.snapshot("npc:doctor-a").activeAction());
        assertEquals("life_extension", YidaoNpcAiStateStore.snapshot("npc:doctor-b").activeAction());
    }

    @Test
    void healer_npc_ai_clear_payload_removes_only_matching_healer() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor-a","active_action":"idle","queue_len":0,"reputation":1,"retreating":false}
            """, 160);
        router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor-b","active_action":"retreat","queue_len":0,"reputation":-2,"retreating":true}
            """, 160);
        var cleared = router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor-a","active_action":"clear","queue_len":0,"reputation":0,"retreating":false}
            """, 160);

        assertTrue(cleared.isHandled());
        assertFalse(YidaoNpcAiStateStore.snapshot("npc:doctor-a").active());
        assertEquals("retreat", YidaoNpcAiStateStore.snapshot("npc:doctor-b").activeAction());
        assertEquals(1, YidaoNpcAiStateStore.activeCount());
    }

    @Test
    void reputation_fields_keep_negative_values() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        var hud = router.route("""
            {"v":1,"type":"yidao_hud_state","healer_id":"npc:doctor","reputation":-4,"peace_mastery":0.0,"karma":0.0,"active_skill":null,"patient_ids":[],"patient_hp_percent":null,"patient_contam_total":null,"severed_meridian_count":0,"contract_count":0,"mass_preview_count":0}
            """, 256);
        var ai = router.route("""
            {"v":1,"type":"healer_npc_ai_state","healer_id":"npc:doctor","active_action":"retreat","queue_len":0,"reputation":-7,"retreating":true}
            """, 160);

        assertTrue(hud.isHandled());
        assertTrue(ai.isHandled());
        assertEquals(-4, YidaoHudStateStore.snapshot().reputation());
        assertEquals(-7, YidaoNpcAiStateStore.snapshot().reputation());
    }

    @Test
    void empty_hud_projection_clears_active_snapshot() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        router.route("""
            {"v":1,"type":"yidao_hud_state","healer_id":"npc:doctor","reputation":7,"peace_mastery":48.0,"karma":3.5,"active_skill":"life_extension","patient_ids":["offline:Patient"],"patient_hp_percent":0.5,"patient_contam_total":1.25,"severed_meridian_count":1,"contract_count":2,"mass_preview_count":0}
            """, 256);
        var cleared = router.route("""
            {"v":1,"type":"yidao_hud_state","healer_id":"npc:doctor","reputation":0,"peace_mastery":0.0,"karma":0.0,"active_skill":null,"patient_ids":[],"patient_hp_percent":null,"patient_contam_total":null,"severed_meridian_count":0,"contract_count":0,"mass_preview_count":0}
            """, 256);

        assertTrue(cleared.isHandled());
        assertFalse(YidaoHudStateStore.snapshot().active());
    }
}
