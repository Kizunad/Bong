package com.bong.client.season;

import com.bong.client.atmosphere.ZoneAtmosphereRenderer;
import com.bong.client.audio.MusicStateMachine;
import com.bong.client.botany.BotanyPlantVisualState;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.LingtianOverlayHudPlanner;
import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.network.VfxEventPayload;
import com.bong.client.state.SeasonState;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SeasonFullExperienceTest {
    @AfterEach
    void tearDown() {
        SeasonVisualController.resetForTests();
    }

    @Test
    void controller_syncs_all_systems() {
        SeasonState winter = new SeasonState(SeasonState.Phase.WINTER, 250L, 1000L, 0L);

        SeasonVisualController.tick(winter, 42L);

        assertEquals(SeasonState.Phase.WINTER, ZoneAtmosphereRenderer.currentSeasonOverrideForTests().phase());
        assertEquals(0.25, ZoneAtmosphereRenderer.currentSeasonOverrideForTests().progress(), 1e-6);
        assertEquals(SeasonState.Phase.WINTER, MusicStateMachine.instance().seasonModifierForTests().phase());
        assertEquals(0.25, MusicStateMachine.instance().seasonModifierForTests().progress(), 1e-6);
    }

    @Test
    void controller_emits_transition_event_on_phase_change() {
        SeasonState summer = new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L);
        SeasonState tide = new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 10L, 1000L, 0L);

        SeasonVisualController.tick(summer, 1L);
        SeasonVisualController.SeasonTickResult result = SeasonVisualController.tick(tide, 2L);

        assertEquals(SeasonState.Phase.SUMMER, result.transition().from());
        assertEquals(SeasonState.Phase.SUMMER_TO_WINTER, result.transition().to());
    }

    @Test
    void full_season_cycle_keeps_visual_signals_in_sync() {
        SeasonState[] cycle = {
            new SeasonState(SeasonState.Phase.SUMMER, 100L, 1000L, 0L),
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 200L, 1000L, 0L),
            new SeasonState(SeasonState.Phase.WINTER, 300L, 1000L, 0L),
            new SeasonState(SeasonState.Phase.WINTER_TO_SUMMER, 400L, 1000L, 0L),
            new SeasonState(SeasonState.Phase.SUMMER, 500L, 1000L, 1L)
        };

        int transitions = 0;
        for (int i = 0; i < cycle.length; i++) {
            SeasonVisualController.SeasonTickResult result = SeasonVisualController.tick(cycle[i], 100L + i);
            transitions += result.transition() == null ? 0 : 1;
            assertEquals(cycle[i].phase(), ZoneAtmosphereRenderer.currentSeasonOverrideForTests().phase());
            assertEquals(cycle[i].phase(), MusicStateMachine.instance().seasonModifierForTests().phase());
            assertFalse(SeasonHintHudPlanner.buildCommands(cycle[i], 320, 180).isEmpty());
            assertFalse(SeasonParticleEmitter.plan(cycle[i], 120L).isEmpty());
        }

        assertEquals(4, transitions);
    }

    @Test
    void hud_icon_no_text() {
        List<HudRenderCommand> commands = SeasonHintHudPlanner.buildCommands(
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 0L, 1000L, 0L),
            320,
            180
        );

        assertFalse(commands.isEmpty());
        assertTrue(commands.stream().noneMatch(HudRenderCommand::isText));
        assertTrue(commands.stream().allMatch(HudRenderCommand::isRect));
    }

    @Test
    void summer_heat_wave_particles() {
        List<SeasonParticleEmitter.ParticleCue> cues = SeasonParticleEmitter.plan(
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L),
            120L
        );

        assertTrue(cues.stream().anyMatch(cue -> cue.kind() == SeasonParticleEmitter.ParticleKind.HEAT_SHIMMER));
        assertTrue(cues.stream().anyMatch(cue -> cue.kind() == SeasonParticleEmitter.ParticleKind.DISTANT_THUNDER_FLASH));
    }

    @Test
    void winter_snow_drift_speed() {
        SeasonParticleEmitter.ParticleCue snow = SeasonParticleEmitter.plan(
                new SeasonState(SeasonState.Phase.WINTER, 0L, 1000L, 0L),
                120L
            )
            .stream()
            .filter(cue -> cue.kind() == SeasonParticleEmitter.ParticleKind.SNOW_DRIFT)
            .findFirst()
            .orElseThrow();

        assertTrue(snow.yVelocity() < -0.02);
        assertEquals("cloud256_dust", snow.spriteId());
    }

    @Test
    void transition_chaos_particles() {
        List<SeasonParticleEmitter.ParticleCue> cues = SeasonParticleEmitter.plan(
            new SeasonState(SeasonState.Phase.WINTER_TO_SUMMER, 0L, 1000L, 0L),
            120L
        );

        assertTrue(cues.stream().anyMatch(cue -> cue.kind() == SeasonParticleEmitter.ParticleKind.CHAOTIC_QI_LINE));
        assertTrue(cues.stream().anyMatch(cue -> cue.kind() == SeasonParticleEmitter.ParticleKind.TRIBULATION_MARK));
    }

    @Test
    void plant_heat_tolerance_visual() {
        BotanyPlantVisualState base = new BotanyPlantVisualState(1.0f, 255, 0x70AA50, 0.05f);
        BotanyPlantVisualState tolerant = SeasonPlantVisuals.apply(
            "chi_sui_cao",
            base,
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L),
            0L
        );
        BotanyPlantVisualState fragile = SeasonPlantVisuals.apply(
            "ning_mai_cao",
            base,
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L),
            0L
        );

        assertTrue(tolerant.scale() > base.scale());
        assertNotEquals(tolerant.tintRgb(), fragile.tintRgb());
    }

    @Test
    void frost_species_fade_in_winter() {
        BotanyPlantVisualState base = new BotanyPlantVisualState(1.0f, 255, 0x70AA50, 0.05f);
        BotanyPlantVisualState early = SeasonPlantVisuals.apply(
            "xue_po_lian",
            base,
            new SeasonState(SeasonState.Phase.WINTER, 20L, 1000L, 0L),
            0L
        );
        BotanyPlantVisualState later = SeasonPlantVisuals.apply(
            "xue_po_lian",
            base,
            new SeasonState(SeasonState.Phase.WINTER, 400L, 1000L, 0L),
            0L
        );

        assertTrue(early.alpha() < later.alpha());
        assertTrue(SeasonPlantVisuals.isFrostSpecies("xue_po_lian"));
    }

    @Test
    void lingtian_overlay_season_icon() {
        LingtianSessionStore.Snapshot snapshot = new LingtianSessionStore.Snapshot(
            true,
            LingtianSessionStore.Kind.HARVEST,
            1,
            64,
            1,
            25,
            100,
            "凝脉草",
            "manual",
            0.0f,
            false
        );

        List<HudRenderCommand> commands = LingtianOverlayHudPlanner.buildCommands(
            snapshot,
            320,
            180,
            new SeasonState(SeasonState.Phase.WINTER, 0L, 1000L, 0L)
        );

        assertTrue(commands.stream().anyMatch(command -> command.isRect() && command.color() == 0x99E8F4FF));
    }

    @Test
    void migration_dust_particles() {
        MigrationVisualPlanner.MigrationVisualCommand command = MigrationVisualPlanner.plan(
            new MigrationVisualPlanner.MigrationVisualEvent("spawn", 1.0, 0.0, 6000, 96, 100L),
            3100L
        );

        assertEquals(8, command.dustPerEntityPerFiveTicks());
        assertEquals("migration_rumble", command.rumbleRecipeId());
    }

    @Test
    void migration_camera_shake() {
        MigrationVisualPlanner.MigrationVisualCommand command = MigrationVisualPlanner.plan(
            new MigrationVisualPlanner.MigrationVisualEvent("spawn", 1.0, 0.0, 6000, 24, 100L),
            3100L
        );

        assertTrue(command.cameraShakeIntensity() > 0.0);
        assertTrue(command.cameraShakeIntensity() <= 0.05);
    }

    @Test
    void migration_vfx_payload_reaches_visual_planner() {
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            new Identifier("bong", "migration_visual"),
            new double[] { 0.0, 64.0, 0.0 },
            Optional.of(new double[] { 1.0, 0.0, 0.0 }),
            OptionalInt.empty(),
            Optional.of(0.8),
            OptionalInt.of(36),
            OptionalInt.of(200)
        );

        MigrationVisualPlanner.MigrationVisualCommand command = MigrationVisualPlanner.plan(
            MigrationVisualPlanner.fromVfxPayload(payload, 1_000L),
            1_000L
        );

        assertTrue(command.dustPerEntityPerFiveTicks() > 0);
        assertTrue(command.cameraShakeIntensity() > 0.0);
    }

    @Test
    void summer_breakthrough_extra_lightning() {
        SeasonBreakthroughOverlay.BreakthroughProfile profile = SeasonBreakthroughOverlay.breakthroughProfile(
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L),
            true,
            0L
        );

        assertEquals(1.50, profile.lightningMultiplier(), 1e-6);
        assertEquals(0xFFD36A, profile.pillarTintRgb());
    }

    @Test
    void winter_breakthrough_frost_refraction() {
        SeasonBreakthroughOverlay.BreakthroughProfile profile = SeasonBreakthroughOverlay.breakthroughProfile(
            new SeasonState(SeasonState.Phase.WINTER, 0L, 1000L, 0L),
            true,
            0L
        );

        assertEquals("enlightenment_dust", profile.particleSpriteId());
        assertEquals(0xC0E0FF, profile.pillarTintRgb());
    }

    @Test
    void transition_breakthrough_flicker() {
        SeasonBreakthroughOverlay.BreakthroughProfile left = SeasonBreakthroughOverlay.breakthroughProfile(
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 0L, 1000L, 0L),
            true,
            0L
        );
        SeasonBreakthroughOverlay.BreakthroughProfile right = SeasonBreakthroughOverlay.breakthroughProfile(
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 0L, 1000L, 0L),
            true,
            4L
        );

        assertNotEquals(left.pillarTintRgb(), right.pillarTintRgb());
        assertTrue(left.backlashIntensity() > 0.5);
    }

    @Test
    void breakthrough_profile_pulse_reaches_hud() {
        SeasonBreakthroughOverlay.BreakthroughProfile profile = SeasonBreakthroughOverlay.breakthroughProfile(
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 0L, 1000L, 0L),
            true,
            0L
        );

        SeasonBreakthroughOverlayHud.trigger(profile, 1_000L);
        List<HudRenderCommand> commands = SeasonBreakthroughOverlayHud.buildCommands(2_000L);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isScreenTint));
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isEdgeVignette));
    }

    @Test
    void meditation_absorb_density_by_season() {
        SeasonBreakthroughOverlay.MeditationProfile summer = SeasonBreakthroughOverlay.meditationAbsorbProfile(
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 1000L, 0L),
            0L
        );
        SeasonBreakthroughOverlay.MeditationProfile winter = SeasonBreakthroughOverlay.meditationAbsorbProfile(
            new SeasonState(SeasonState.Phase.WINTER, 0L, 1000L, 0L),
            0L
        );

        assertTrue(summer.densityMultiplier() > 1.0);
        assertTrue(winter.densityMultiplier() < 1.0);
    }
}
