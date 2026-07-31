package com.bong.client.social;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NicheGuardianStoreTest {
    @AfterEach
    void tearDown() {
        NicheGuardianStore.resetForTests();
    }

    @Test
    void panelListsGuardianStatusAndRecentIntrusion() {
        NicheGuardianStore.recordFatigue("puppet", 4);
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            List.of(42L),
            "char:raider",
            0.2,
            1000L
        ));

        List<String> lines = NicheGuardianPanel.buildLines();

        assertEquals("puppet x4", lines.get(0));
        assertTrue(lines.get(1).contains("char:raider"));
    }

    @Test
    void clearOnDisconnectDropsOldGuardianDataAndAllowsNewSessionWrites() {
        NicheGuardianStore.recordFatigue("old_puppet", 4);
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            List.of(42L),
            "char:old_raider",
            0.2,
            1000L
        ));

        NicheGuardianStore.clearOnDisconnect();

        assertTrue(NicheGuardianStore.guardianStatuses().isEmpty(),
            "断线生产清理必须清空旧会话的守护状态，避免 HUD 跨世界残留");
        assertTrue(NicheGuardianStore.intrusionAlerts().isEmpty(),
            "断线生产清理必须清空旧会话的入侵告警，避免新会话显示旧盗取记录");

        NicheGuardianStore.recordBroken("new_trap", "char:new_raider");

        assertEquals(1, NicheGuardianStore.guardianStatuses().size(),
            "清空后新会话应能写入新的守护状态");
        assertTrue(NicheGuardianStore.guardianStatuses().containsKey("new_trap"),
            "新会话写入不得被旧断线状态阻塞");
        assertEquals(1, NicheGuardianStore.intrusionAlerts().size(),
            "新会话破损事件应只产生新的入侵告警");
        assertEquals("char:new_raider", NicheGuardianStore.intrusionAlerts().get(0).intruderId(),
            "清空后的告警必须来自新会话，不能混入旧会话数据");
    }
}
