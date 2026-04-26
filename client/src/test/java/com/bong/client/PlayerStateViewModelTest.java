package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class PlayerStateViewModelTest {
    @BeforeEach
    void setUp() {
        PlayerStateState.clear();
    }

    @AfterEach
    void tearDown() {
        PlayerStateState.clear();
    }

    @Test
    public void emptyViewModelUsesReadOnlyFallbacks() {
        PlayerStateViewModel viewModel = PlayerStateViewModel.fromCurrentState();

        assertFalse(viewModel.hasState());
        assertEquals("尚未收到 player_state 载荷", viewModel.statusText());
        assertEquals("未感应", viewModel.realmLabel());
        assertEquals("0 / 0", viewModel.spiritQiLabel());
        assertEquals("░░░░░░░░░░", viewModel.spiritQiBar());
        assertEquals("+0.00", viewModel.karmaLabel());
        assertEquals("善 ════●════ 恶", viewModel.karmaAxis());
        assertEquals("0.00", viewModel.compositePowerLabel());
        assertEquals("未知区域", viewModel.zoneLabel());
        assertEquals("OFF", viewModel.dynamicXmlUiLabel());
    }

    @Test
    public void viewModelFormatsCultivationFieldsForScreenDisplay() {
        PlayerStateState.record(
                new BongServerPayload.PlayerState("Induce", 78.0d, 100.0d, -0.2d, 0.35d, "blood_valley"),
                10_000L
        );

        PlayerStateViewModel viewModel = PlayerStateViewModel.fromCurrentState();

        assertTrue(viewModel.hasState());
        assertEquals("引气", viewModel.realmLabel());
        assertEquals("78 / 100", viewModel.spiritQiLabel());
        assertEquals("████████░░", viewModel.spiritQiBar());
        assertEquals("-0.20", viewModel.karmaLabel());
        assertEquals("善 ═══●═════ 恶", viewModel.karmaAxis());
        assertEquals("0.35", viewModel.compositePowerLabel());
        assertEquals("Blood Valley", viewModel.zoneLabel());
        assertEquals(4, viewModel.powerBreakdown().size());
        assertEquals("战斗", viewModel.powerBreakdown().get(0).label());
        assertEquals("0.40", viewModel.powerBreakdown().get(0).valueLabel());
        assertEquals("财富", viewModel.powerBreakdown().get(1).label());
        assertEquals("0.42", viewModel.powerBreakdown().get(1).valueLabel());
        assertEquals("社交", viewModel.powerBreakdown().get(2).label());
        assertEquals("0.38", viewModel.powerBreakdown().get(2).valueLabel());
        assertEquals("领地", viewModel.powerBreakdown().get(3).label());
        assertEquals("0.27", viewModel.powerBreakdown().get(3).valueLabel());
    }
}
