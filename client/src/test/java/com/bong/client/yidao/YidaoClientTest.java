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
        YidaoNpcAiStateStore.Snapshot snapshot = YidaoNpcAiStateStore.snapshot();
        assertEquals("npc:doctor", snapshot.healerId());
        assertEquals("meridian_repair", snapshot.activeAction());
        assertEquals(3, snapshot.queueLen());
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
