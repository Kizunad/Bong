package com.bong.client.audio;

import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.network.AudioEventPayload;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.function.Predicate;

public final class SoundRecipePlayer implements com.bong.client.network.AudioPlaybackBridge {
    private static final int MAX_PLAYS_PER_TICK = 4;
    private static final int MAX_ONE_SHOTS_PER_TICK = 3;
    private static final int PREEMPT_PRIORITY = 85;
    private static final int DUCK_TRANSITION_TICKS = 40;
    private static final float COMBAT_AMBIENT_VOLUME = 0.3f;

    private static final SoundRecipePlayer INSTANCE =
        new SoundRecipePlayer(new MinecraftSoundSink(), SoundRecipePlayer::defaultFlagActive);

    private final SoundSink sink;
    private final Predicate<String> flagProvider;
    private final Map<Long, ActiveLoop> loops = new LinkedHashMap<>();
    private final List<AudioEventPayload.PlaySoundRecipe> pending = new ArrayList<>();
    private float ambientVolumeFactor = 1.0f;
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
        payload.recipe().loop().ifPresent(loop -> {
            String whileFlag = payload.flag().orElse(loop.whileFlag());
            payload.flag().ifPresent(EnvironmentAudioLoopState::activate);
            loops.put(payload.instanceId(), new ActiveLoop(
                payload,
                tick + loop.intervalTicks(),
                whileFlag,
                payload.flag().orElse(null)
            ));
        });
        enqueue(payload);
        return true;
    }

    @Override
    public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
        ActiveLoop removed = loops.remove(payload.instanceId());
        if (removed != null) {
            removed.deactivateOwnedFlag();
        }
        sink.stop(payload.instanceId(), payload.fadeOutTicks());
        return true;
    }

    public void tick() {
        tick++;
        updateAmbientDucking();
        Iterator<Map.Entry<Long, ActiveLoop>> iterator = loops.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<Long, ActiveLoop> entry = iterator.next();
            ActiveLoop active = entry.getValue();
            if (!flagProvider.test(active.whileFlag)) {
                active.deactivateOwnedFlag();
                iterator.remove();
                continue;
            }
            if (tick >= active.nextTick) {
                enqueue(active.payload);
                active.nextTick = tick + active.payload.recipe().loop()
                    .map(AudioLoopConfig::intervalTicks)
                    .orElse(Integer.MAX_VALUE);
            }
        }
        drainPending();
    }

    public int activeLoopCountForTests() {
        return loops.size();
    }

    private void enqueue(AudioEventPayload.PlaySoundRecipe payload) {
        if (payload.recipe().priority() >= PREEMPT_PRIORITY) {
            loops.entrySet().removeIf(entry -> {
                boolean sameCategory = entry.getValue().payload.recipe().category() == payload.recipe().category();
                boolean lowerPriority = entry.getValue().payload.recipe().priority() < payload.recipe().priority();
                if (sameCategory && lowerPriority) {
                    entry.getValue().deactivateOwnedFlag();
                    sink.stop(entry.getKey(), 0);
                    return true;
                }
                return false;
            });
        }
        pending.add(payload);
    }

    private void drainPending() {
        if (pending.isEmpty()) {
            return;
        }
        pending.sort(Comparator
            .comparingInt((AudioEventPayload.PlaySoundRecipe payload) -> payload.recipe().priority())
            .reversed());

        List<AudioEventPayload.PlaySoundRecipe> selected = new ArrayList<>(MAX_PLAYS_PER_TICK);
        AudioEventPayload.PlaySoundRecipe selectedLoop = null;
        int oneShots = 0;
        for (AudioEventPayload.PlaySoundRecipe payload : pending) {
            if (payload.recipe().loop().isPresent()) {
                if (selectedLoop == null) {
                    selectedLoop = payload;
                }
                continue;
            }
            if (oneShots < MAX_ONE_SHOTS_PER_TICK) {
                selected.add(payload);
                oneShots++;
            }
        }
        if (selectedLoop != null && selected.size() < MAX_PLAYS_PER_TICK) {
            selected.add(selectedLoop);
        }

        for (AudioEventPayload.PlaySoundRecipe payload : selected) {
            playLayers(payload);
        }
        pending.clear();
    }

    private boolean playLayers(AudioEventPayload.PlaySoundRecipe payload) {
        boolean anyPlayed = false;
        for (AudioLayer layer : payload.recipe().layers()) {
            float volume = layer.volume() * payload.volumeMul();
            if (payload.recipe().category() == AudioCategory.AMBIENT) {
                volume *= ambientVolumeFactor;
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
        if (EnvironmentAudioLoopState.isActive(flag)) {
            return true;
        }
        return switch (flag) {
            case "hp_below_30" -> CombatHudStateStore.snapshot().hpPercent() < 0.3f;
            case "lingtian_drain_active" -> {
                LingtianSessionStore.Snapshot snapshot = LingtianSessionStore.snapshot();
                yield snapshot.active() && snapshot.kind() == LingtianSessionStore.Kind.DRAIN_QI;
            }
            default -> false;
        };
    }

    private void updateAmbientDucking() {
        float target = CombatHudStateStore.snapshot().active() ? COMBAT_AMBIENT_VOLUME : 1.0f;
        float step = (1.0f - COMBAT_AMBIENT_VOLUME) / DUCK_TRANSITION_TICKS;
        if (ambientVolumeFactor < target) {
            ambientVolumeFactor = Math.min(target, ambientVolumeFactor + step);
        } else if (ambientVolumeFactor > target) {
            ambientVolumeFactor = Math.max(target, ambientVolumeFactor - step);
        }
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static final class ActiveLoop {
        final AudioEventPayload.PlaySoundRecipe payload;
        final String whileFlag;
        final String ownedFlag;
        long nextTick;

        ActiveLoop(
            AudioEventPayload.PlaySoundRecipe payload,
            long nextTick,
            String whileFlag,
            String ownedFlag
        ) {
            this.payload = payload;
            this.nextTick = nextTick;
            this.whileFlag = whileFlag;
            this.ownedFlag = ownedFlag;
        }

        void deactivateOwnedFlag() {
            if (ownedFlag != null) {
                EnvironmentAudioLoopState.deactivate(ownedFlag);
            }
        }
    }
}
