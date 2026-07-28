package com.bong.client.audio;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.hud.HudImmersionMode;
import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.BongClient;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.tiandao.TiandaoPresenceState;
import com.bong.client.tiandao.TiandaoPresenceStore;
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
    private static final int UI_RESTORE_ON_DAMAGE_TICKS = 100;
    private static final float COMBAT_AMBIENT_VOLUME = 0.3f;
    private static final float AUDIO_PITCH_MIN = 0.1f;
    private static final float AUDIO_PITCH_MAX = 2.0f;

    private static final SoundRecipePlayer INSTANCE =
        new SoundRecipePlayer(new MinecraftSoundSink(), SoundRecipePlayer::defaultFlagActive);

    private final SoundSink sink;
    private final Predicate<String> flagProvider;
    private final AudioBusMixer mixer;
    private final AudioTelemetry telemetry;
    private final Map<Long, ActiveLoop> loops = new LinkedHashMap<>();
    private final List<AudioEventPayload.PlaySoundRecipe> pending = new ArrayList<>();
    private float ambientVolumeFactor = 1.0f;
    private boolean lastCombatActive;
    private float lastCombatHpPercent = Float.NaN;
    private long tick;

    public SoundRecipePlayer(SoundSink sink, Predicate<String> flagProvider) {
        this(sink, flagProvider, new AudioBusMixer(), new AudioTelemetry());
    }

    public SoundRecipePlayer(
        SoundSink sink,
        Predicate<String> flagProvider,
        AudioBusMixer mixer,
        AudioTelemetry telemetry
    ) {
        this.sink = Objects.requireNonNull(sink, "sink");
        this.flagProvider = Objects.requireNonNull(flagProvider, "flagProvider");
        this.mixer = Objects.requireNonNull(mixer, "mixer");
        this.telemetry = Objects.requireNonNull(telemetry, "telemetry");
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
            ActiveLoop previous = loops.put(payload.instanceId(), new ActiveLoop(
                payload,
                tick + loop.intervalTicks(),
                whileFlag,
                payload.flag().orElse(null)
            ));
            if (previous != null) {
                previous.deactivateOwnedFlag();
                sink.stop(payload.instanceId(), 0);
            }
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
        CombatHudState combat = CombatHudStateStore.snapshot();
        mixer.setImmersiveMode(HudImmersionMode.immersiveActive());
        restoreUiOnCombatEdge(combat);
        mixer.tick();
        updateAmbientDucking(combat);
        Iterator<Map.Entry<Long, ActiveLoop>> iterator = loops.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<Long, ActiveLoop> entry = iterator.next();
            ActiveLoop active = entry.getValue();
            if (!flagProvider.test(active.whileFlag)) {
                active.deactivateOwnedFlag();
                sink.stop(entry.getKey(), 0);
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

    int pendingCountForTests() {
        return pending.size();
    }

    long tickForTests() {
        return tick;
    }

    float ambientVolumeFactorForTests() {
        return ambientVolumeFactor;
    }

    /**
     * 断线时撤销本 session 的所有声音派生状态。
     *
     * <p>先从 player 摘掉全部旧会话引用和派生 flag，再 best-effort 硬停 sink 中的实例；
     * sink 的 {@link RuntimeException} 只记录，不得阻断本地状态复位或中央 helper 的后续
     * animation/HUD 清理。{@link Error} 仍原样透传。
     * mixer、telemetry、flag provider 与 sink 依赖均保留，保证同一 player 在新 session 可重用。
     */
    public void clearOnDisconnect() {
        List<Long> staleInstanceIds = new ArrayList<>(loops.keySet());
        loops.clear();
        pending.clear();
        EnvironmentAudioLoopState.clearOnDisconnect();
        ambientVolumeFactor = 1.0f;
        lastCombatActive = false;
        lastCombatHpPercent = Float.NaN;
        tick = 0L;
        mixer.clearOnDisconnect();

        for (Long instanceId : staleInstanceIds) {
            try {
                sink.stop(instanceId, 0);
            } catch (RuntimeException exception) {
                BongClient.LOGGER.error(
                    "Failed to stop sound instance {} while clearing disconnect state",
                    instanceId,
                    exception
                );
            }
        }
        try {
            sink.clearOnDisconnect();
        } catch (RuntimeException exception) {
            BongClient.LOGGER.error("Failed to clear sound sink on disconnect", exception);
        }
    }

    public void setMusicState(MusicStateMachine.State state) {
        mixer.setMusicState(state);
    }

    public AudioBusMixer mixerForTests() {
        return mixer;
    }

    public AudioTelemetry telemetryForTests() {
        return telemetry;
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
            volume *= mixer.effectiveVolume(payload.recipe().bus());
            float pitch = (float) clamp(layer.pitch() * Math.pow(2.0, payload.pitchShift()), AUDIO_PITCH_MIN, AUDIO_PITCH_MAX);
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
        int count = telemetry.record(payload.recipeId(), System.currentTimeMillis());
        if (count == 101 && telemetry.isOverThreshold(payload.recipeId(), System.currentTimeMillis())) {
            BongClient.LOGGER.warn("[bong][audio] recipe {} played more than 100 times in 30 min", payload.recipeId());
        }
        return anyPlayed;
    }

    private static boolean defaultFlagActive(String flag) {
        if (flag != null && flag.startsWith("tiandao:")) {
            String response = flag.substring("tiandao:".length());
            TiandaoPresenceState state = TiandaoPresenceStore.snapshot();
            return state.active() && state.response().equals(response);
        }
        // 有内置状态谓词的 flag（低血 / 灵田抽灵）必须**先**按真实状态判定，不能被
        // payload 自注册的 sticky flag 短路成永真：server 发带 flag 的 loop 时
        // play() 会 EnvironmentAudioLoopState.activate(flag)，若让 sticky 优先，
        // recipe 的 while_flag 就成了死条件——heartbeat_low_hp 的
        // `minecraft:entity.player.hurt` 层会在血量回满（含重生）后仍每秒重放。
        // 与上面 `tiandao:` 前缀同一治法（见 tiandaoFlagFollowsPresenceState... 测试）。
        Boolean stateDriven = stateDrivenFlagActive(flag);
        if (stateDriven != null) {
            return stateDriven;
        }
        // 无内置谓词的 flag（环境雾堤 / fauna 压迫感 hum 等）仍由 server 的
        // play…stop 配对拥有生命周期。
        return EnvironmentAudioLoopState.isActive(flag);
    }

    /** 内置状态谓词；返回 null 表示该 flag 无内置状态、生命周期归 server 的 play/stop。 */
    private static Boolean stateDrivenFlagActive(String flag) {
        if (flag == null) {
            return Boolean.FALSE;
        }
        return switch (flag) {
            case "hp_below_20" -> CombatHudStateStore.snapshot().hpPercent() < 0.2f;
            case "hp_below_30" -> CombatHudStateStore.snapshot().hpPercent() < 0.3f;
            case "lingtian_drain_active" -> {
                LingtianSessionStore.Snapshot snapshot = LingtianSessionStore.snapshot();
                yield snapshot.active() && snapshot.kind() == LingtianSessionStore.Kind.DRAIN_QI;
            }
            default -> null;
        };
    }

    static boolean defaultFlagActiveForTests(String flag) {
        return defaultFlagActive(flag);
    }

    private void restoreUiOnCombatEdge(CombatHudState combat) {
        boolean active = combat.active();
        float hpPercent = combat.hpPercent();
        boolean hpDropped = active
            && lastCombatActive
            && Float.isFinite(lastCombatHpPercent)
            && hpPercent < lastCombatHpPercent;
        if (active && (!lastCombatActive || hpDropped)) {
            mixer.restoreUiForTicks(UI_RESTORE_ON_DAMAGE_TICKS);
        }
        lastCombatActive = active;
        lastCombatHpPercent = hpPercent;
    }

    private void updateAmbientDucking(CombatHudState combat) {
        float target = combat.active() ? COMBAT_AMBIENT_VOLUME : 1.0f;
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
