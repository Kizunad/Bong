package com.bong.client.state;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class PlayerStateViewModelTest {
    @Test
    void emptyFactoryReturnsUnsyncedDefaults() {
        PlayerStateViewModel state = PlayerStateViewModel.empty();

        assertTrue(state.isEmpty());
        assertEquals("", state.realm());
        assertEquals("", state.playerId());
        assertEquals(0.0, state.spiritQiCurrent());
        assertEquals(100.0, state.spiritQiMax());
        assertEquals(0.0, state.spiritQiFillRatio());
        assertEquals(0.0, state.karma());
        assertEquals(0.0, state.compositePower());
        assertEquals(0.0, state.breakdown().combat());
        assertEquals(0.0, state.zoneSpiritQiNormalized());
    }

    @Test
    void createClampsOutOfRangeValuesIntoRenderSafeState() {
        PlayerStateViewModel state = PlayerStateViewModel.create(
            " Induce ",
            " offline:Azure ",
            150.0,
            80.0,
            1.6,
            -0.25,
            PlayerStateViewModel.PowerBreakdown.create(1.4, -0.5, Double.NaN, 0.75),
            PlayerStateViewModel.SocialSnapshot.create(7, 12, List.of("背盟者"), "defend", 0, 10, 1),
            " azure_peak ",
            "   ",
            4.0
        );

        assertEquals("Induce", state.realm());
        assertEquals("offline:Azure", state.playerId());
        assertEquals(80.0, state.spiritQiCurrent(), 0.0001);
        assertEquals(80.0, state.spiritQiMax(), 0.0001);
        assertEquals(1.0, state.spiritQiFillRatio(), 0.0001);
        assertEquals(1.0, state.karma(), 0.0001);
        assertEquals(0.0, state.compositePower(), 0.0001);
        assertEquals(1.0, state.breakdown().combat(), 0.0001);
        assertEquals(0.0, state.breakdown().wealth(), 0.0001);
        assertEquals(0.0, state.breakdown().social(), 0.0001);
        assertEquals(0.75, state.breakdown().territory(), 0.0001);
        assertEquals(7, state.social().fame());
        assertEquals(12, state.social().notoriety());
        assertEquals(List.of("背盟者"), state.social().topTags());
        assertEquals("defend", state.social().faction());
        assertEquals("azure_peak", state.zoneId());
        assertEquals("azure_peak", state.zoneLabel());
        assertEquals(1.0, state.zoneSpiritQiNormalized(), 0.0001);
    }

    @Test
    void invalidSpiritQiMaxFallsBackToSafeDefaultAndNullBreakdown() {
        PlayerStateViewModel state = PlayerStateViewModel.create(
            "Condense",
            null,
            45.0,
            0.0,
            -1.6,
            0.6,
            null,
            null,
            "",
            "Qingyun Peak",
            -0.4
        );

        assertEquals(45.0, state.spiritQiCurrent(), 0.0001);
        assertEquals(100.0, state.spiritQiMax(), 0.0001);
        assertEquals(0.45, state.spiritQiFillRatio(), 0.0001);
        assertEquals(-1.0, state.karma(), 0.0001);
        assertEquals("Qingyun Peak", state.zoneId());
        assertEquals("Qingyun Peak", state.zoneLabel());
        assertEquals(0.0, state.zoneSpiritQiNormalized(), 0.0001);
        assertEquals(0.0, state.breakdown().combat(), 0.0001);
        assertTrue(state.social().topTags().isEmpty());
        assertFalse(state.social().hasFaction());
    }

    @Test
    void blankRealmTransitionsBackToEmptyState() {
        PlayerStateViewModel state = PlayerStateViewModel.create(
            "   ",
            "offline:Azure",
            25.0,
            100.0,
            0.2,
            0.4,
            PlayerStateViewModel.PowerBreakdown.create(0.1, 0.2, 0.3, 0.4),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "qingyun_peak",
            "Qingyun Peak",
            0.5
        );

        assertTrue(state.isEmpty());
        assertEquals(0.0, state.spiritQiFillRatio());
    }
}
