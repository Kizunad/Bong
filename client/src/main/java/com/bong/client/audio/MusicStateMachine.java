package com.bong.client.audio;

import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.state.SeasonState;

import java.util.Locale;
import java.util.Objects;
import java.util.Optional;

public final class MusicStateMachine {
    private static final long INSTANCE_ID_BASE = 60_000L;
    private static final int MAX_ABS_BLOCK_POS = 30_000_000;
    private static final float MAX_VOLUME_MUL = 4.0f;
    private static final MusicStateMachine INSTANCE = new MusicStateMachine(SoundRecipePlayer.instance());

    private final SoundRecipePlayer player;
    private long nextInstanceId = INSTANCE_ID_BASE;
    private ActiveMusic active;
    private SeasonModifier seasonModifier = new SeasonModifier(SeasonState.Phase.SUMMER, 0.0);

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

    public void setSeasonModifier(SeasonState.Phase phase, double progress) {
        seasonModifier = new SeasonModifier(
            phase == null ? SeasonState.Phase.SUMMER : phase,
            clamp01(progress)
        );
    }

    public SeasonModifier seasonModifierForTests() {
        return seasonModifier;
    }

    public void clearSeasonModifierForTests() {
        seasonModifier = new SeasonModifier(SeasonState.Phase.SUMMER, 0.0);
    }

    private void stopActive(int fadeTicks) {
        if (active == null) {
            return;
        }
        active.loopFlag.ifPresent(EnvironmentAudioLoopState::deactivate);
        player.stop(new AudioEventPayload.StopSoundRecipe(active.instanceId, Math.max(0, fadeTicks)));
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
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
                return Optional.of(State.valueOf(raw.trim().toUpperCase(Locale.ROOT)));
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
            if (season == null || season.isBlank()) {
                throw new IllegalArgumentException("season must not be blank");
            }
            tsyDepth = tsyDepth == null ? Optional.empty() : tsyDepth;
            tsyDepth.ifPresent(depth -> {
                if (!isTsyDepth(depth)) {
                    throw new IllegalArgumentException("tsyDepth must be shallow, mid, or deep");
                }
            });
            if (fadeTicks < 0) {
                throw new IllegalArgumentException("fadeTicks must be >= 0");
            }
            pos = pos == null ? Optional.empty() : pos;
            pos.ifPresent(position -> {
                if (!isBlockPosInProtocolRange(position)) {
                    throw new IllegalArgumentException("pos must be within Minecraft block coordinate range");
                }
            });
            if (!Float.isFinite(volumeMul) || volumeMul < 0.0f || volumeMul > MAX_VOLUME_MUL) {
                throw new IllegalArgumentException("volumeMul out of range");
            }
            if (!Float.isFinite(pitchShift) || pitchShift < -1.0f || pitchShift > 1.0f) {
                throw new IllegalArgumentException("pitchShift out of range");
            }
            Objects.requireNonNull(recipe, "recipe");
            if (!ambientRecipeId.equals(recipe.id())) {
                throw new IllegalArgumentException("ambientRecipeId must match recipe.id");
            }
        }
    }

    private static boolean isTsyDepth(String depth) {
        return switch (depth) {
            case "shallow", "mid", "deep" -> true;
            default -> false;
        };
    }

    private static boolean isBlockPosInProtocolRange(AudioPosition pos) {
        return Math.abs(pos.x()) <= MAX_ABS_BLOCK_POS
            && Math.abs(pos.y()) <= MAX_ABS_BLOCK_POS
            && Math.abs(pos.z()) <= MAX_ABS_BLOCK_POS;
    }

    private record ActiveMusic(long instanceId, TransitionKey key, Optional<String> loopFlag) {
    }

    public record SeasonModifier(SeasonState.Phase phase, double progress) {
    }

    private record TransitionKey(
        String zoneName,
        String recipeId,
        State state,
        boolean night,
        String season,
        Optional<String> tsyDepth,
        float volumeMul,
        float pitchShift,
        AudioRecipe recipe
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
                update.pitchShift(),
                update.recipe()
            );
        }
    }
}
