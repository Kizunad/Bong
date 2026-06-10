package com.bong.client.entity;

import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Arrays;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

public final class BongEntityRegistry {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong/entity-model");

    private BongEntityRegistry() {}

    public static EntityType<BongModeledEntity> type(BongEntityModelKind modelKind) {
        EntityType<BongModeledEntity> type = Holder.TYPES.get(modelKind);
        if (type == null) {
            throw new IllegalStateException("Entity type is not registered yet for " + modelKind);
        }
        return type;
    }

    /**
     * Validates raw ids assigned during Holder class initialization.
     *
     * <p>The actual Fabric registration happens when {@link Holder#TYPES} is
     * initialized; this method deliberately fails fast if a later bootstrap
     * change shifts the server-facing protocol ids.
     */
    public static void register() {
        registerKinds(baseKindsForTests());
    }

    /**
     * Registers entity model kinds that must come after another bootstrap.
     *
     * <p>Workbench must stay raw_id=165 because Baolongwang occupies 164 on both
     * server and client. Keeping it deferred avoids shifting either contract.</p>
     */
    public static void registerDeferred() {
        registerKinds(deferredKindsForTests());
    }

    public static List<BongEntityModelKind> orderedKindsForTests() {
        return List.of(BongEntityModelKind.values());
    }

    public static List<BongEntityModelKind> baseKindsForTests() {
        return Arrays.stream(BongEntityModelKind.values())
            .filter(kind -> kind != BongEntityModelKind.WORKBENCH)
            .toList();
    }

    public static List<BongEntityModelKind> deferredKindsForTests() {
        return List.of(BongEntityModelKind.WORKBENCH);
    }

    public static Map<BongEntityModelKind, Integer> expectedRawIdsForTests() {
        EnumMap<BongEntityModelKind, Integer> ids = new EnumMap<>(BongEntityModelKind.class);
        for (BongEntityModelKind kind : BongEntityModelKind.values()) {
            ids.put(kind, kind.expectedRawId());
        }
        return Map.copyOf(ids);
    }

    private static void registerKinds(List<BongEntityModelKind> kinds) {
        for (BongEntityModelKind kind : kinds) {
            EntityType<BongModeledEntity> existing = Holder.TYPES.get(kind);
            EntityType<BongModeledEntity> type = existing == null ? registerType(kind) : existing;
            int rawId = Registries.ENTITY_TYPE.getRawId(type);
            if (rawId != kind.expectedRawId()) {
                LOGGER.error(
                    "[bong][entity-model] raw_id MISMATCH: {} expected {} actual {}. "
                        + "Keep BongEntityRenderBootstrap after WhaleRenderBootstrap and FaunaRenderBootstrap, "
                        + "and deferred workbench after BaolongwangRenderBootstrap.",
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
        }
    }

    private static EntityType<BongModeledEntity> registerType(BongEntityModelKind kind) {
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
        Holder.TYPES.put(kind, type);
        return type;
    }

    private static final class Holder {
        private static final EnumMap<BongEntityModelKind, EntityType<BongModeledEntity>> TYPES =
            new EnumMap<>(BongEntityModelKind.class);
    }
}
