package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.bong.client.visual.realm_vision.SenseKind;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class HudImmersionMatrixTest {
    private static final HudTextHelper.WidthMeasurer WIDTH = text -> text == null ? 0 : text.length() * 6;
    private static final String[] REALMS = {"Awaken", "Induce", "Condense", "Solidify", "Spirit", "Void"};
    private static final Resolution[] RESOLUTIONS = {
        new Resolution(1920, 1080),
        new Resolution(1366, 768),
        new Resolution(2560, 1440)
    };

    @AfterEach
    void reset() {
        PlayerStateStore.resetForTests();
        PerceptionEdgeStateStore.replace(PerceptionEdgeState.empty());
        HudImmersionMode.resetForTests();
        HudLayoutPreferenceStore.resetForTests();
    }

    @Test
    void matrixCoversLayoutRealmImmersiveEnvironmentAndResolution() {
        long totalNanos = 0L;
        int visited = 0;
        int measured = 0;
        for (HudLayoutPreset preset : HudLayoutPreset.values()) {
            for (String realm : REALMS) {
                for (boolean immersive : new boolean[]{false, true}) {
                    for (EnvironmentCase environment : EnvironmentCase.values()) {
                        for (Resolution resolution : RESOLUTIONS) {
                            HudImmersionMode.resetForTests();
                            PlayerStateStore.replace(player(realm, environment.zone));
                            PerceptionEdgeStateStore.replace(threatState());
                            HudImmersionMode.setManualImmersive(immersive, 1_000L);
                            long start = System.nanoTime();
                            List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
                                snapshot(environment.zone, preset),
                                combatFor(preset),
                                1_600L,
                                WIDTH,
                                220,
                                resolution.width,
                                resolution.height,
                                null,
                                runtime()
                            );
                            long elapsed = System.nanoTime() - start;
                            if (visited >= 144) {
                                totalNanos += elapsed;
                                measured++;
                            }
                            visited++;
                            assertNewHudCommandsStayInBounds(commands, resolution);
                        }
                    }
                }
            }
        }
        double avgMillis = totalNanos / 1_000_000.0 / Math.max(1, measured);
        assertTrue(avgMillis < 1.5, "HUD matrix average build should stay under 1.5ms, was " + avgMillis);
    }

    private static BongHudStateSnapshot snapshot(ZoneState zone, HudLayoutPreset preset) {
        VisualEffectState visual = preset == HudLayoutPreset.CULTIVATION
            ? VisualEffectState.create("meditation_calm", 1.0, 10_000L, 1_000L)
            : VisualEffectState.none();
        return BongHudStateSnapshot.create(zone, NarrationState.empty(), visual);
    }

    private static CombatHudSnapshot combatFor(HudLayoutPreset preset) {
        if (preset != HudLayoutPreset.COMBAT) {
            return CombatHudSnapshot.empty();
        }
        return CombatHudSnapshot.create(
            CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none()),
            null,
            com.bong.client.combat.QuickSlotConfig.empty(),
            com.bong.client.combat.SkillBarConfig.empty(),
            -1,
            com.bong.client.combat.CastState.idle(),
            com.bong.client.combat.UnifiedEventStream.empty(),
            com.bong.client.combat.SpellVolumeState.idle(),
            com.bong.client.combat.store.CarrierStateStore.State.NONE,
            com.bong.client.combat.DefenseWindowState.idle(),
            com.bong.client.combat.UnlockedStyles.none()
        );
    }

    private static PlayerStateViewModel player(String realm, ZoneState zone) {
        return PlayerStateViewModel.create(
            realm,
            "offline:test",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            zone.zoneId(),
            zone.zoneLabel(),
            zone.spiritQiNormalized(),
            zone.negativeSpiritQi() ? zone.spiritQiRaw() : 0.0
        );
    }

    private static PerceptionEdgeState threatState() {
        return new PerceptionEdgeState(
            List.of(new PerceptionEdgeState.SenseEntry(SenseKind.CRISIS_PREMONITION, 0.0, 64.0, 8.0, 0.9)),
            1L
        );
    }

    private static HudRuntimeContext runtime() {
        return new HudRuntimeContext(0.0, 0.0, 64.0, 0.0, false, List.of());
    }

    private static void assertNewHudCommandsStayInBounds(List<HudRenderCommand> commands, Resolution resolution) {
        for (HudRenderCommand command : commands) {
            if (!isNewHudLayer(command.layer()) || !command.isRect() || command.width() <= 0 || command.height() <= 0) {
                continue;
            }
            assertTrue(command.x() >= -4, "x underflow " + command);
            assertTrue(command.y() >= -4, "y underflow " + command);
            assertTrue(command.x() + command.width() <= resolution.width + 4, "x overflow " + command);
            assertTrue(command.y() + command.height() <= resolution.height + 4, "y overflow " + command);
        }
    }

    private static boolean isNewHudLayer(HudRenderLayer layer) {
        return layer == HudRenderLayer.QI_RADAR
            || layer == HudRenderLayer.COMPASS
            || layer == HudRenderLayer.THREAT_INDICATOR
            || layer == HudRenderLayer.HUD_VARIANT;
    }

    private enum EnvironmentCase {
        NORMAL(ZoneState.create("jade", "青谷", 0.8, 1, 0L)),
        NEGATIVE(ZoneState.create("negative", "负灵域", -0.5, 3, "normal", 0L)),
        DEAD(ZoneState.create("dead", "荒死地", 0.0, 4, "collapsed", 0L)),
        TSY(ZoneState.create("tsy_lingxu", "坍缩渊", 0.3, 5, 0L));

        private final ZoneState zone;

        EnvironmentCase(ZoneState zone) {
            this.zone = zone;
        }
    }

    private record Resolution(int width, int height) {
    }
}
