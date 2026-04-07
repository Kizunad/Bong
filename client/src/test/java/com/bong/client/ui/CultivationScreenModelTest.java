package com.bong.client.ui;

import com.bong.client.PlayerStateCache;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CultivationScreenModelTest {
    @Test
    void formatsSyncedSnapshotForCultivationScreen() {
        CultivationScreenModel model = CultivationScreenModel.from(
            new PlayerStateCache.PlayerStateSnapshot(
                "qi_refining_3",
                78.0,
                0.2,
                0.35,
                new PlayerStateCache.PowerBreakdown(0.2, 0.4, 0.65, 0.2, 0.1),
                "qingyun_peak"
            )
        );

        assertTrue(model.synced());
        assertEquals("练气三层", model.realmLabel());
        assertEquals("78 / 100", model.spiritQiText());
        assertEquals(0.78, model.spiritQiRatio(), 1e-9);
        assertEquals("+0.20", model.karmaText());
        assertEquals(0.6, model.karmaRatio(), 1e-9);
        assertEquals("0.35", model.compositePowerText());
        assertEquals("Qingyun Peak", model.zoneText());
        assertEquals(5, model.breakdownEntries().size());
        assertEquals("战斗", model.breakdownEntries().get(0).label());
        assertEquals("0.20", model.breakdownEntries().get(0).valueText());
    }

    @Test
    void usesFallbackValuesWhenPlayerStateIsMissing() {
        CultivationScreenModel model = CultivationScreenModel.from(null);

        assertFalse(model.synced());
        assertEquals("未同步", model.realmLabel());
        assertEquals("0 / 100", model.spiritQiText());
        assertEquals(0.0, model.spiritQiRatio(), 1e-9);
        assertEquals("+0.00", model.karmaText());
        assertEquals(0.5, model.karmaRatio(), 1e-9);
        assertEquals("未知区域", model.zoneText());
        assertEquals("等待 server 下发 player_state", model.footerText());
    }
}
