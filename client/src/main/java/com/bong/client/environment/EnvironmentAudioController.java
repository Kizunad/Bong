package com.bong.client.environment;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioLoopConfig;
import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.network.AudioEventPayload;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

public final class EnvironmentAudioController {
    private final Map<String, ActiveLoop> loops = new LinkedHashMap<>();
    private long nextInstanceId = 30_000L;

    public void update(Collection<ActiveEmitter> activeEmitters, Vec3d playerPos) {
        Set<String> keep = new HashSet<>();
        for (ActiveEmitter emitter : activeEmitters) {
            if (emitter.alpha() <= 0.05f || emitter.fadingOut()) {
                continue;
            }
            String recipeId = emitter.behavior().ambientLoopRecipe(emitter.effect());
            if (recipeId == null || recipeId.isBlank()) {
                continue;
            }
            keep.add(emitter.key());
            loops.computeIfAbsent(emitter.key(), key -> startLoop(key, recipeId, emitter.effect(), playerPos));
        }

        List<String> stop = new ArrayList<>();
        for (String key : loops.keySet()) {
            if (!keep.contains(key)) {
                stop.add(key);
            }
        }
        for (String key : stop) {
            stopLoop(key);
        }
    }

    public int activeLoopCountForTests() {
        return loops.size();
    }

    public void clear() {
        for (String key : List.copyOf(loops.keySet())) {
            stopLoop(key);
        }
        loops.clear();
        EnvironmentAudioLoopState.clear();
    }

    private ActiveLoop startLoop(String key, String recipeId, EnvironmentEffect effect, Vec3d playerPos) {
        long instanceId = ++nextInstanceId;
        String flag = loopFlag(key);
        EnvironmentAudioLoopState.activate(flag);
        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            recipeId,
            instanceId,
            Optional.of(positionFrom(effect.anchor(), playerPos)),
            Optional.of(flag),
            0.75f,
            0.0f,
            recipe(recipeId, flag)
        ));
        return new ActiveLoop(instanceId, flag, effect.fadeOutTicks());
    }

    static String loopFlag(String key) {
        return "zone_env:" + key;
    }

    private void stopLoop(String key) {
        ActiveLoop loop = loops.remove(key);
        if (loop == null) {
            return;
        }
        EnvironmentAudioLoopState.deactivate(loop.flag);
        SoundRecipePlayer.instance().stop(new AudioEventPayload.StopSoundRecipe(
            loop.instanceId,
            loop.fadeOutTicks
        ));
    }

    private static AudioPosition positionFrom(Vec3d anchor, Vec3d fallback) {
        Vec3d pos = anchor == null ? fallback : anchor;
        if (pos == null) {
            pos = Vec3d.ZERO;
        }
        return new AudioPosition((int) Math.floor(pos.x), (int) Math.floor(pos.y), (int) Math.floor(pos.z));
    }

    private static AudioRecipe recipe(String recipeId, String flag) {
        return new AudioRecipe(
            recipeId,
            List.of(new AudioLayer(soundFor(recipeId), volumeFor(recipeId), pitchFor(recipeId), 0)),
            Optional.of(new AudioLoopConfig(60, flag)),
            priorityFor(recipeId),
            AudioAttenuation.ZONE_BROADCAST,
            AudioCategory.AMBIENT
        );
    }

    private static Identifier soundFor(String recipeId) {
        String path = switch (recipeId) {
            case "thunder_distant_loop" -> "entity.lightning_bolt.thunder";
            case "static_crackle_loop" -> "block.fire.ambient";
            case "wind_cold_loop", "wind_dry_loop", "wind_howl_loop" -> "weather.rain";
            default -> "ambient.cave";
        };
        return new Identifier("minecraft", path);
    }

    private static float volumeFor(String recipeId) {
        return switch (recipeId) {
            case "thunder_distant_loop" -> 0.22f;
            case "static_crackle_loop" -> 0.18f;
            default -> 0.14f;
        };
    }

    private static float pitchFor(String recipeId) {
        return switch (recipeId) {
            case "wind_cold_loop" -> 0.82f;
            case "wind_dry_loop" -> 1.12f;
            default -> 1.0f;
        };
    }

    private static int priorityFor(String recipeId) {
        return "thunder_distant_loop".equals(recipeId) ? 36 : 24;
    }

    private record ActiveLoop(long instanceId, String flag, int fadeOutTicks) {
    }
}
