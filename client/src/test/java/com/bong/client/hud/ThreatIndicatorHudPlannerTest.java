package com.bong.client.hud;

import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.SenseKind;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ThreatIndicatorHudPlannerTest {
    @Test
    void threatIndicatorHiddenBelowTongling() {
        List<HudRenderCommand> commands = ThreatIndicatorHudPlanner.buildCommands(
            player("Solidify"),
            threatState(16.0, 0.8),
            TribulationStateStore.State.NONE,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        assertTrue(commands.isEmpty());
    }

    @Test
    void pulseFrequencyByDistance() {
        assertEquals(300L, ThreatIndicatorHudPlanner.pulsePeriodMs(0.0));
        assertEquals(1_000L, ThreatIndicatorHudPlanner.pulsePeriodMs(64.0));
    }

    @Test
    void lockWarningOneSecond() {
        List<HudRenderCommand> commands = ThreatIndicatorHudPlanner.buildCommands(
            player("Spirit"),
            threatState(4.0, 1.0),
            TribulationStateStore.State.NONE,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        long fullEdgeRects = commands.stream()
            .filter(HudRenderCommand::isRect)
            .filter(cmd -> cmd.layer() == HudRenderLayer.THREAT_INDICATOR)
            .count();
        assertTrue(fullEdgeRects >= 4);
    }

    @Test
    void voidRealmAddsAttentionBar() {
        List<HudRenderCommand> commands = ThreatIndicatorHudPlanner.buildCommands(
            player("Void"),
            threatState(10.0, 0.7),
            TribulationStateStore.State.NONE,
            HudRuntimeContext.empty(),
            1_000L,
            320,
            180
        );

        assertTrue(commands.stream().anyMatch(cmd ->
            cmd.isRect() && (cmd.color() & 0x00FFFFFF) == (ThreatIndicatorHudPlanner.ATTENTION_FILL & 0x00FFFFFF)
        ));
    }

    private static PerceptionEdgeState threatState(double z, double intensity) {
        return new PerceptionEdgeState(
            List.of(new PerceptionEdgeState.SenseEntry(SenseKind.CRISIS_PREMONITION, 0.0, 64.0, z, intensity)),
            1L
        );
    }

    private static PlayerStateViewModel player(String realm) {
        return PlayerStateViewModel.create(
            realm,
            "offline:test",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "jade",
            "青谷",
            0.5
        );
    }
}
