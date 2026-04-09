package com.bong.client.visual;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VisualEffectSystemTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void warningPathBuildsShakeLikeTextAndExpiresOnLifecycle() {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("screen_shake", 1.0, 8_000L, 0L),
            1_000L,
            true
        );

        List<HudRenderCommand> initialCommands = VisualEffectPlanner.buildCommands(
            acceptedState,
            1_000L,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        );
        List<HudRenderCommand> shiftedCommands = VisualEffectPlanner.buildCommands(
            acceptedState,
            1_090L,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        );

        assertEquals(VisualEffectState.EffectType.SCREEN_SHAKE, acceptedState.effectType());
        assertEquals(VisualEffectProfile.SYSTEM_WARNING.maxDurationMillis(), acceptedState.durationMillis());
        assertEquals(VisualEffectProfile.SYSTEM_WARNING.maxIntensity(), acceptedState.intensity(), 0.0001);
        assertEquals(1, initialCommands.size());
        assertTrue(initialCommands.get(0).isText());
        assertTrue(initialCommands.get(0).text().contains("天道警示"));
        assertNotEquals(initialCommands.get(0).x(), shiftedCommands.get(0).x());
        assertTrue(VisualEffectPlanner.buildCommands(acceptedState, 3_500L, FIXED_WIDTH, 220, 320, 180, true).isEmpty());
    }

    @Test
    void perceptionPathBuildsConservativeTintOverlay() {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("fog_tint", 1.0, 12_000L, 0L),
            200L,
            true
        );

        List<HudRenderCommand> commands = VisualEffectPlanner.buildCommands(
            acceptedState,
            200L,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        );

        assertEquals(1, commands.size());
        assertTrue(commands.get(0).isScreenTint());
        assertEquals(VisualEffectProfile.PERCEPTION.baseColor(), commands.get(0).color() & 0x00FFFFFF);
        assertTrue((commands.get(0).color() >>> 24) > 0);
        assertTrue((commands.get(0).color() >>> 24) < 100);
    }

    @Test
    void eraDecreePathBuildsGoldTitleAndFadesBeforeExpiry() {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("title_flash", 1.0, 10_000L, 0L),
            400L,
            true
        );

        HudRenderCommand initialCommand = VisualEffectPlanner.buildCommands(
            acceptedState,
            400L,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        ).get(0);
        HudRenderCommand fadedCommand = VisualEffectPlanner.buildCommands(
            acceptedState,
            2_000L,
            FIXED_WIDTH,
            220,
            320,
            180,
            true
        ).get(0);

        assertTrue(initialCommand.isText());
        assertTrue(initialCommand.text().contains("时代法旨"));
        assertEquals(VisualEffectProfile.ERA_DECREE.baseColor(), initialCommand.color() & 0x00FFFFFF);
        assertTrue((initialCommand.color() >>> 24) > (fadedCommand.color() >>> 24));
        assertTrue(VisualEffectPlanner.buildCommands(acceptedState, 3_700L, FIXED_WIDTH, 220, 320, 180, true).isEmpty());
    }

    @Test
    void repeatedTriggersStayDebouncedAndBounded() {
        VisualEffectState acceptedState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("screen_shake", 5.0, 12_000L, 0L),
            0L,
            true
        );
        VisualEffectState repeatedState = acceptedState;
        for (long nowMillis = 100L; nowMillis <= 1_100L; nowMillis += 100L) {
            repeatedState = VisualEffectController.acceptIncoming(
                repeatedState,
                VisualEffectState.create("screen_shake", 5.0, 12_000L, nowMillis),
                nowMillis,
                true
            );
        }

        VisualEffectState cooledDownState = VisualEffectController.acceptIncoming(
            repeatedState,
            VisualEffectState.create("screen_shake", 0.7, 12_000L, 1_300L),
            1_300L,
            true
        );

        assertEquals(0L, repeatedState.startedAtMillis());
        assertEquals(VisualEffectProfile.SYSTEM_WARNING.maxDurationMillis(), repeatedState.durationMillis());
        assertEquals(VisualEffectProfile.SYSTEM_WARNING.maxIntensity(), repeatedState.intensity(), 0.0001);
        assertEquals(1_300L, cooledDownState.startedAtMillis());
        assertEquals(VisualEffectProfile.SYSTEM_WARNING.maxDurationMillis(), cooledDownState.durationMillis());
        assertEquals(0.7, cooledDownState.intensity(), 0.0001);
    }

    @Test
    void disabledAndUnknownEffectsProduceSafeNoOpBehavior() {
        VisualEffectState currentState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("fog_tint", 0.4, 1_500L, 0L),
            100L,
            true
        );
        VisualEffectState disabledState = VisualEffectController.acceptIncoming(
            currentState,
            VisualEffectState.create("title_flash", 1.0, 2_000L, 0L),
            200L,
            false
        );
        VisualEffectState unknownState = VisualEffectController.acceptIncoming(
            currentState,
            VisualEffectState.create("unknown_effect", 0.9, 2_000L, 0L),
            300L,
            true
        );

        assertEquals(currentState.effectType(), disabledState.effectType());
        assertEquals(currentState.startedAtMillis(), disabledState.startedAtMillis());
        assertEquals(currentState.effectType(), unknownState.effectType());
        assertTrue(VisualEffectPlanner.buildCommands(currentState, 100L, FIXED_WIDTH, 220, 320, 180, false).isEmpty());
        assertFalse(currentState.isEmpty());
    }
}
