package com.bong.client.audio;

import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.network.AudioEventPayload;

import java.util.Objects;
import java.util.Optional;

public final class MusicStateMachine {
    private static final long INSTANCE_ID_BASE = 60_000L;
    private static final MusicStateMachine INSTANCE = new MusicStateMachine(SoundRecipePlayer.instance());

    private final SoundRecipePlayer player;
    private long nextInstanceId = INSTANCE_ID_BASE;
    private ActiveMusic active;

    public MusicStateMachine(SoundRecipePlayer player) {
        this.player = Objects.requireNonNull(player, "player");
    }

    public static MusicStateMachine instance() {
        return INSTANCE;
    }

    public boolean apply(AmbientZoneUpdate update) {
        Objects.requireNonNull(update, "update");
        TransitionKey nextKey = TransitionKey.from(update);
        if (active != null && active.key.equals(nextKey)) {
            return false;
        }

        stopActive(update.fadeTicks());
        long instanceId = ++nextInstanceId;
        update.recipe().loop().map(AudioLoopConfig::whileFlag).ifPresent(EnvironmentAudioLoopState::activate);
        player.play(new AudioEventPayload.PlaySoundRecipe(
            update.ambientRecipeId(),
            instanceId,
            update.pos(),
            update.recipe().loop().map(AudioLoopConfig::whileFlag),
            update.volumeMul(),
            update.pitchShift(),
            update.recipe()
        ));
        active = new ActiveMusic(instanceId, nextKey, update.recipe().loop().map(AudioLoopConfig::whileFlag));
        return true;
    }

    public void clear() {
        stopActive(0);
        active = null;
    }

    public State currentStateForTests() {
        return active == null ? null : active.key.state;
    }

    public long activeInstanceIdForTests() {
        return active == null ? 0L : active.instanceId;
    }

    private void stopActive(int fadeTicks) {
        if (active == null) {
            return;
        }
        active.loopFlag.ifPresent(EnvironmentAudioLoopState::deactivate);
        player.stop(new AudioEventPayload.StopSoundRecipe(active.instanceId, Math.max(0, fadeTicks)));
    }

    public enum State {
        AMBIENT(0),
        CULTIVATION(1),
        TSY(2),
        COMBAT(3),
        TRIBULATION(4);

        private final int priority;

        State(int priority) {
            this.priority = priority;
        }

        public int priority() {
            return priority;
        }

        public static Optional<State> fromWire(String raw) {
            if (raw == null || raw.isBlank()) {
                return Optional.empty();
            }
            try {
                return Optional.of(State.valueOf(raw.trim().toUpperCase()));
            } catch (IllegalArgumentException ignored) {
                return Optional.empty();
            }
        }

        public static State resolve(boolean tribulation, boolean combat, boolean tsy, boolean cultivation) {
            if (tribulation) {
                return TRIBULATION;
            }
            if (combat) {
                return COMBAT;
            }
            if (tsy) {
                return TSY;
            }
            if (cultivation) {
                return CULTIVATION;
            }
            return AMBIENT;
        }
    }

    public record AmbientZoneUpdate(
        String zoneName,
        String ambientRecipeId,
        State state,
        boolean night,
        String season,
        Optional<String> tsyDepth,
        int fadeTicks,
        Optional<AudioPosition> pos,
        float volumeMul,
        float pitchShift,
        AudioRecipe recipe
    ) {
        public AmbientZoneUpdate {
            if (zoneName == null || zoneName.isBlank()) {
                throw new IllegalArgumentException("zoneName must not be blank");
            }
            if (ambientRecipeId == null || ambientRecipeId.isBlank()) {
                throw new IllegalArgumentException("ambientRecipeId must not be blank");
            }
            Objects.requireNonNull(state, "state");
            season = season == null ? "" : season;
            tsyDepth = tsyDepth == null ? Optional.empty() : tsyDepth;
            pos = pos == null ? Optional.empty() : pos;
            Objects.requireNonNull(recipe, "recipe");
        }
    }

    private record ActiveMusic(long instanceId, TransitionKey key, Optional<String> loopFlag) {
    }

    private record TransitionKey(
        String zoneName,
        String recipeId,
        State state,
        boolean night,
        String season,
        Optional<String> tsyDepth,
        float volumeMul,
        float pitchShift
    ) {
        static TransitionKey from(AmbientZoneUpdate update) {
            return new TransitionKey(
                update.zoneName(),
                update.ambientRecipeId(),
                update.state(),
                update.night(),
                update.season(),
                update.tsyDepth(),
                update.volumeMul(),
                update.pitchShift()
            );
        }
    }
}
