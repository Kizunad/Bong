package com.bong.client.season;

import com.bong.client.atmosphere.ZoneAtmosphereRenderer;
import com.bong.client.audio.MusicStateMachine;
import com.bong.client.hud.BongToast;
import com.bong.client.state.SeasonState;
import com.bong.client.state.SeasonStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;

import java.util.Objects;
import java.util.function.Consumer;

public final class SeasonVisualController {
    private static boolean bootstrapped;
    private static SeasonState.Phase lastPhase;
    private static boolean firstHintShown;
    private static Consumer<SeasonTransitionEvent> transitionSink = event -> {};

    private SeasonVisualController() {
    }

    public static void register() {
        if (bootstrapped) {
            return;
        }
        bootstrapped = true;
        ClientTickEvents.END_CLIENT_TICK.register(SeasonVisualController::onEndClientTick);
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.world == null) {
            return;
        }
        SeasonState state = SeasonStateStore.snapshot();
        long worldTick = client.world.getTime();
        SeasonState safeState = state == null ? SeasonState.summerAt(worldTick) : state;
        if (state != null && !firstHintShown) {
            firstHintShown = true;
            BongToast.show("你感到天地间灵气在变化", 0xFFE0D0AA, System.currentTimeMillis(), 3500L);
        }
        tick(safeState, worldTick);
        SeasonParticleEmitter.updateSeason(client, safeState, worldTick);
    }

    public static SeasonTickResult tick(SeasonState state, long worldTick) {
        return tick(state, worldTick, event -> {});
    }

    private static SeasonTickResult tick(
        SeasonState state,
        long worldTick,
        Consumer<SeasonTransitionEvent> transitionSideEffect
    ) {
        SeasonState safeState = state == null ? SeasonState.summerAt(worldTick) : state;
        double progress = progress(safeState);
        ZoneAtmosphereRenderer.setSeasonOverride(safeState.phase(), progress);
        MusicStateMachine.instance().setSeasonModifier(safeState.phase(), progress);

        SeasonTransitionEvent event = null;
        if (lastPhase != null && lastPhase != safeState.phase()) {
            event = new SeasonTransitionEvent(lastPhase, safeState.phase(), progress, worldTick);
            transitionSink.accept(event);
            transitionSideEffect.accept(event);
        }
        lastPhase = safeState.phase();
        return new SeasonTickResult(safeState.phase(), progress, event);
    }

    public static void setTransitionSinkForTests(Consumer<SeasonTransitionEvent> sink) {
        transitionSink = Objects.requireNonNull(sink, "sink");
    }

    public static void resetForTests() {
        lastPhase = null;
        firstHintShown = false;
        transitionSink = event -> {};
        SeasonBreakthroughOverlayHud.resetForTests();
        ZoneAtmosphereRenderer.clearSeasonOverrideForTests();
        MusicStateMachine.instance().clearSeasonModifierForTests();
    }

    private static double progress(SeasonState state) {
        return Math.max(0.0, Math.min(1.0, (double) state.tickIntoPhase() / (double) state.phaseTotalTicks()));
    }

    public record SeasonTickResult(
        SeasonState.Phase phase,
        double progress,
        SeasonTransitionEvent transition
    ) {
    }
}
