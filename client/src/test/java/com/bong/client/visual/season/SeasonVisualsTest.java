package com.bong.client.visual.season;

import com.bong.client.state.SeasonState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

class SeasonVisualsTest {
    @Test
    void qiBarColorUsesIndirectSeasonSaturation() {
        int base = 0xCC40C0E0;

        int summer = SeasonVisuals.qiBarColor(
            base,
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1_382_400L, 0L),
            0L
        );
        int winter = SeasonVisuals.qiBarColor(
            base,
            new SeasonState(SeasonState.Phase.WINTER, 0L, 1_382_400L, 0L),
            0L
        );

        assertNotEquals(base, summer);
        assertNotEquals(base, winter);
        assertNotEquals(summer, winter);
    }

    @Test
    void tideTurnHasTintButNoSignatureParticle() {
        SeasonState tideTurn = new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 0L, 345_600L, 0L);

        assertNotEquals(0, SeasonVisuals.skyTintArgb(tideTurn, 0L));
        assertEquals(SeasonVisuals.ParticleKind.NONE, SeasonVisuals.particleCue(tideTurn, 120L).kind());
    }

    @Test
    void stablePhasesProduceDistinctParticleCuesOnCadence() {
        assertEquals(
            SeasonVisuals.ParticleKind.HEAT_SHIMMER,
            SeasonVisuals.particleCue(new SeasonState(SeasonState.Phase.SUMMER, 0L, 1_382_400L, 0L), 120L).kind()
        );
        assertEquals(
            SeasonVisuals.ParticleKind.SNOW_GRAIN,
            SeasonVisuals.particleCue(new SeasonState(SeasonState.Phase.WINTER, 0L, 1_382_400L, 0L), 120L).kind()
        );
    }
}
