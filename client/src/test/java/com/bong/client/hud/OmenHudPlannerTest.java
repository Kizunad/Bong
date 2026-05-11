package com.bong.client.hud;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.omen.OmenStateStore;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OmenHudPlannerTest {
    @AfterEach
    void reset() {
        OmenStateStore.resetForTests();
    }

    @Test
    void pseudoVeinOmenRendersNonTextualEdgeVignette() {
        OmenStateStore.note(payload("world_omen_pseudo_vein", 0.7), 1_000L);

        List<HudRenderCommand> commands = OmenHudPlanner.buildCommands(
            OmenStateStore.snapshot(1_500L),
            1_500L,
            320,
            180
        );

        assertFalse(commands.isEmpty());
        assertTrue(commands.stream().allMatch(cmd -> !cmd.isText() && !cmd.isScaledText()));
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isEdgeVignette));
    }

    @Test
    void beastTideOmenUsesBottomDustBand() {
        OmenStateStore.note(payload("world_omen_beast_tide", 0.8), 1_000L);

        List<HudRenderCommand> commands = OmenHudPlanner.buildCommands(
            OmenStateStore.snapshot(1_200L),
            1_200L,
            320,
            180
        );

        HudRenderCommand band = commands.stream()
            .filter(HudRenderCommand::isRect)
            .findFirst()
            .orElseThrow();
        assertEquals(0, band.x());
        assertEquals(162, band.y());
        assertEquals(320, band.width());
    }

    @Test
    void omenStateExpires() {
        OmenStateStore.note(payload("world_omen_karma_backlash", 1.0), 1_000L);

        assertFalse(OmenStateStore.snapshot(1_500L).entries().isEmpty());
        assertTrue(OmenStateStore.snapshot(20_000L).entries().isEmpty());
    }

    private static VfxEventPayload.SpawnParticle payload(String path, double strength) {
        return new VfxEventPayload.SpawnParticle(
            new Identifier("bong", path),
            new double[] { 0.0, 64.0, 0.0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.of(strength),
            OptionalInt.of(12),
            OptionalInt.of(200)
        );
    }
}
