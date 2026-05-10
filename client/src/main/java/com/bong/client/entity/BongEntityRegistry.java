package com.bong.client.entity;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

public final class BongEntityRegistry {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/entity-model");

    private BongEntityRegistry() {}

    public static EntityType<BongModeledEntity> type(BongEntityModelKind modelKind) {
        return Holder.TYPES.get(modelKind);
    }

    public static void register() {
        Holder.TYPES.forEach((kind, type) -> {
            int rawId = Registries.ENTITY_TYPE.getRawId(type);
            if (rawId != kind.expectedRawId()) {
                LOGGER.error(
                    "[bong][entity-model] raw_id MISMATCH: {} expected {} actual {}. "
                        + "Keep BongEntityRenderBootstrap after WhaleRenderBootstrap, or update server EntityKind ids.",
                    kind.identifier(),
                    kind.expectedRawId(),
                    rawId
                );
                throw new IllegalStateException(
                    "Entity raw_id mismatch for " + kind.identifier()
                        + " expected=" + kind.expectedRawId()
                        + " actual=" + rawId
                );
            }
            LOGGER.info("[bong][entity-model] registered {} raw_id={}", kind.identifier(), rawId);
        });
    }

    public static List<BongEntityModelKind> orderedKindsForTests() {
        return List.of(BongEntityModelKind.values());
    }

    public static Map<BongEntityModelKind, Integer> expectedRawIdsForTests() {
        EnumMap<BongEntityModelKind, Integer> ids = new EnumMap<>(BongEntityModelKind.class);
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            ids.put(kind, kind.expectedRawId());
        }
        return Map.copyOf(ids);
    }

    private static final class Holder {
        private static final EnumMap<BongEntityModelKind, EntityType<BongModeledEntity>> TYPES = createTypes();

        private static EnumMap<BongEntityModelKind, EntityType<BongModeledEntity>> createTypes() {
            EnumMap<BongEntityModelKind, EntityType<BongModeledEntity>> types =
                new EnumMap<>(BongEntityModelKind.class);
            for (BongEntityModelKind kind : BongEntityModelKind.values()) {
                EntityType<BongModeledEntity> type = Registry.register(
                    Registries.ENTITY_TYPE,
                    kind.identifier(),
                    EntityType.Builder
                        .create(BongModeledEntity.factory(kind), SpawnGroup.MISC)
                        .setDimensions(kind.dimensions().width, kind.dimensions().height)
                        .maxTrackingRange(kind.trackingRange())
                        .trackingTickInterval(kind.trackingTickInterval())
                        .disableSaving()
                        .disableSummon()
                        .build(kind.identifier().toString())
                );
                types.put(kind, type);
            }
            return types;
        }
    }
}
