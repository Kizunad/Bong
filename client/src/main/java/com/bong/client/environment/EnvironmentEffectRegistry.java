package com.bong.client.environment;

import com.bong.client.environment.emitter.AshFallEmitter;
import com.bong.client.environment.emitter.DustDevilEmitter;
import com.bong.client.environment.emitter.EmberDriftEmitter;
import com.bong.client.environment.emitter.FogVeilEmitter;
import com.bong.client.environment.emitter.HeatHazeEmitter;
import com.bong.client.environment.emitter.LightningPillarEmitter;
import com.bong.client.environment.emitter.SnowDriftEmitter;
import com.bong.client.environment.emitter.TornadoEmitter;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class EnvironmentEffectRegistry {
    private final Map<String, EmitterBehavior> behaviors = new LinkedHashMap<>();
    private final Map<String, ZoneEnvironmentState> statesByZone = new LinkedHashMap<>();
    private final Map<String, ActiveEmitter> activeByKey = new LinkedHashMap<>();

    public void registerBuiltInBehaviors() {
        registerBehavior("tornado_column", new TornadoEmitter());
        registerBehavior("lightning_pillar", new LightningPillarEmitter());
        registerBehavior("ash_fall", new AshFallEmitter());
        registerBehavior("fog_veil", new FogVeilEmitter());
        registerBehavior("dust_devil", new DustDevilEmitter());
        registerBehavior("ember_drift", new EmberDriftEmitter());
        registerBehavior("heat_haze", new HeatHazeEmitter());
        registerBehavior("snow_drift", new SnowDriftEmitter());
    }

    public void registerBehavior(String kind, EmitterBehavior behavior) {
        if (kind == null || kind.isBlank() || behavior == null) {
            return;
        }
        behaviors.put(kind.trim(), behavior);
    }

    public int behaviorCount() {
        return behaviors.size();
    }

    public void onZoneStateUpdate(ZoneEnvironmentState state) {
        if (state == null || !state.valid()) {
            return;
        }
        statesByZone.put(state.zoneId(), state);

        Set<String> nextKeys = new HashSet<>();
        for (EnvironmentEffect effect : state.effects()) {
            EmitterBehavior behavior = behaviors.get(effect.kind());
            if (behavior == null) {
                continue;
            }
            String key = activeKey(state.zoneId(), effect);
            nextKeys.add(key);
            ActiveEmitter active = activeByKey.get(key);
            if (active == null) {
                activeByKey.put(key, new ActiveEmitter(key, state.zoneId(), effect, behavior, state.generation()));
            } else {
                active.refresh(effect, behavior, state.generation());
            }
        }

        for (ActiveEmitter active : activeByKey.values()) {
            if (active.zoneId().equals(state.zoneId()) && !nextKeys.contains(active.key())) {
                active.markFadingOut();
            }
        }
    }

    public Collection<ActiveEmitter> activeNearPlayer(Vec3d playerPos, double radius) {
        if (playerPos == null) {
            return List.of();
        }
        List<ActiveEmitter> result = new ArrayList<>();
        for (ActiveEmitter active : activeByKey.values()) {
            if (active.alpha() <= 0.0f) {
                continue;
            }
            double limit = Math.min(radius, active.effect().viewRadius());
            if (active.effect().isNear(playerPos, limit)) {
                result.add(active);
            }
        }
        return result;
    }

    public Collection<ActiveEmitter> activeEmitters() {
        return List.copyOf(activeByKey.values());
    }

    public ZoneEnvironmentState zoneState(String zoneId) {
        return statesByZone.get(zoneId);
    }

    public void tickFade() {
        tickFade(null, Double.POSITIVE_INFINITY);
    }

    public void tickFade(Vec3d playerPos, double radius) {
        List<String> remove = new ArrayList<>();
        for (ActiveEmitter active : activeByKey.values()) {
            double limit = Math.min(radius, active.effect().viewRadius());
            boolean inRadius = playerPos == null || active.effect().isNear(playerPos, limit);
            if (!active.advanceFade(inRadius)) {
                remove.add(active.key());
            }
        }
        for (String key : remove) {
            activeByKey.remove(key);
        }
    }

    public void clear() {
        statesByZone.clear();
        activeByKey.clear();
    }

    private static String activeKey(String zoneId, EnvironmentEffect effect) {
        return zoneId + "@" + effect.stableKey();
    }
}
