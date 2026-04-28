package com.bong.client.audio;

import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.network.AudioEventPayload;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;

import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.function.Predicate;

public final class SoundRecipePlayer implements com.bong.client.network.AudioPlaybackBridge {
    private static final SoundRecipePlayer INSTANCE =
        new SoundRecipePlayer(new MinecraftSoundSink(), SoundRecipePlayer::defaultFlagActive);

    private final SoundSink sink;
    private final Predicate<String> flagProvider;
    private final Map<Long, ActiveLoop> loops = new LinkedHashMap<>();
    private long tick;

    public SoundRecipePlayer(SoundSink sink, Predicate<String> flagProvider) {
        this.sink = Objects.requireNonNull(sink, "sink");
        this.flagProvider = Objects.requireNonNull(flagProvider, "flagProvider");
    }

    public static SoundRecipePlayer instance() {
        return INSTANCE;
    }

    public static void bootstrap() {
        ClientTickEvents.END_CLIENT_TICK.register(client -> INSTANCE.tick());
    }

    @Override
    public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
        boolean played = playLayers(payload);
        payload.recipe().loop().ifPresent(loop -> loops.put(payload.instanceId(), new ActiveLoop(
            payload,
            tick + loop.intervalTicks()
        )));
        return played;
    }

    @Override
    public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
        loops.remove(payload.instanceId());
        sink.stop(payload.instanceId(), payload.fadeOutTicks());
        return true;
    }

    public void tick() {
        tick++;
        Iterator<Map.Entry<Long, ActiveLoop>> iterator = loops.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<Long, ActiveLoop> entry = iterator.next();
            ActiveLoop active = entry.getValue();
            String flag = active.payload.recipe().loop().map(AudioLoopConfig::whileFlag).orElse("");
            if (!flagProvider.test(flag)) {
                iterator.remove();
                continue;
            }
            if (tick >= active.nextTick) {
                playLayers(active.payload);
                active.nextTick = tick + active.payload.recipe().loop()
                    .map(AudioLoopConfig::intervalTicks)
                    .orElse(Integer.MAX_VALUE);
            }
        }
    }

    public int activeLoopCountForTests() {
        return loops.size();
    }

    private boolean playLayers(AudioEventPayload.PlaySoundRecipe payload) {
        boolean anyPlayed = false;
        for (AudioLayer layer : payload.recipe().layers()) {
            float volume = layer.volume() * payload.volumeMul();
            if (payload.recipe().category() == AudioCategory.AMBIENT && CombatHudStateStore.snapshot().active()) {
                volume *= 0.3f;
            }
            float pitch = (float) clamp(layer.pitch() * Math.pow(2.0, payload.pitchShift()), 0.5, 2.0);
            anyPlayed |= sink.play(new AudioScheduledSound(
                payload.instanceId(),
                layer.sound(),
                payload.recipe().category(),
                payload.recipe().attenuation(),
                payload.pos(),
                volume,
                pitch,
                layer.delayTicks()
            ));
        }
        return anyPlayed;
    }

    private static boolean defaultFlagActive(String flag) {
        return switch (flag) {
            case "hp_below_30" -> CombatHudStateStore.snapshot().hpPercent() < 0.3f;
            default -> false;
        };
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static final class ActiveLoop {
        final AudioEventPayload.PlaySoundRecipe payload;
        long nextTick;

        ActiveLoop(AudioEventPayload.PlaySoundRecipe payload, long nextTick) {
            this.payload = payload;
            this.nextTick = nextTick;
        }
    }
}
