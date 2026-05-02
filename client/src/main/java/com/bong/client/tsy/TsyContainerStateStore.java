package com.bong.client.tsy;

import net.minecraft.entity.player.PlayerEntity;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class TsyContainerStateStore {
    private static final Map<Long, TsyContainerView> containers = new LinkedHashMap<>();

    private TsyContainerStateStore() {
    }

    public static synchronized void upsert(TsyContainerView view) {
        if (view == null || view.entityId() < 0) {
            return;
        }
        containers.put(view.entityId(), view);
    }

    public static synchronized TsyContainerView get(long entityId) {
        return containers.get(entityId);
    }

    public static synchronized List<TsyContainerView> snapshot() {
        return List.copyOf(new ArrayList<>(containers.values()));
    }

    public static TsyContainerView nearestInteractable(PlayerEntity player, double maxDistance) {
        if (player == null) {
            return null;
        }
        return nearestInteractable(player.getX(), player.getY(), player.getZ(), maxDistance);
    }

    public static synchronized TsyContainerView nearestInteractable(
        double x,
        double y,
        double z,
        double maxDistance
    ) {
        double maxDistanceSq = maxDistance * maxDistance;
        return containers.values().stream()
            .filter(TsyContainerView::interactable)
            .filter(view -> view.distanceSq(x, y, z) <= maxDistanceSq)
            .min(Comparator
                .comparingDouble((TsyContainerView view) -> view.distanceSq(x, y, z))
                .thenComparingLong(TsyContainerView::entityId))
            .orElse(null);
    }

    public static synchronized void resetForTests() {
        containers.clear();
    }
}
