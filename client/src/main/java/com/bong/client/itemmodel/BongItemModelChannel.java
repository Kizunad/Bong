package com.bong.client.itemmodel;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

import net.fabricmc.fabric.api.client.model.loading.v1.ModelLoadingPlugin;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.BakedModelManager;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.client.util.ModelIdentifier;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * P0 spike for the Bong-owned {@code template_id -> BakedModel} channel.
 *
 * <p>This is deliberately additive. The legacy weapon registry and its vanilla
 * stack path remain in place until the P0 decision is accepted by the owner.
 * This class is the only source of truth for the spike model ids and does not
 * select or register a vanilla item host.</p>
 */
public final class BongItemModelChannel {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-item-model-channel");

    /** One real Bong-owned model used to prove the loader and baked lookup path. */
    private static final ModelDefinition WOODEN_SHIELD = new ModelDefinition(
        "wooden_shield",
        Optional.of(modelId("item/wooden_shield/wooden_shield")),
        Optional.empty()
    );

    /**
     * A real legacy template recorded as an explicit geometry-borrow candidate.
     * It is intentionally not loadable yet: the candidate needs its own Bong
     * model definition and transforms before it can enter the production map.
     */
    private static final ModelDefinition QING_FENG_SWORD_BORROW_CANDIDATE = new ModelDefinition(
        "qing_feng_sword",
        Optional.empty(),
        Optional.of("iron_sword")
    );

    private static final Map<String, ModelDefinition> DEFINITIONS;
    private static boolean registered;

    static {
        Map<String, ModelDefinition> definitions = new LinkedHashMap<>();
        definitions.put(WOODEN_SHIELD.templateId(), WOODEN_SHIELD);
        definitions.put(QING_FENG_SWORD_BORROW_CANDIDATE.templateId(), QING_FENG_SWORD_BORROW_CANDIDATE);
        DEFINITIONS = Map.copyOf(definitions);
    }

    private BongItemModelChannel() {
    }

    /**
     * Registers the model loader plugin. Fabric calls the callback again on
     * every resource reload, so the model is re-added to each fresh bake.
     */
    public static void register() {
        if (registered) {
            return;
        }
        registered = true;
        ModelLoadingPlugin.register(context -> DEFINITIONS.values().stream()
            .flatMap(definition -> definition.modelId().stream())
            .forEach(context::addModels));
        LOGGER.info("Bong item model channel registered: {}", WOODEN_SHIELD.modelId().orElseThrow());
    }

    /** Returns the canonical P0 definition, without a fallback to another id. */
    public static Optional<ModelDefinition> definition(String templateId) {
        if (templateId == null || templateId.isBlank()) {
            return Optional.empty();
        }
        return Optional.ofNullable(DEFINITIONS.get(templateId));
    }

    /**
     * Resolves a template against the current baked-model manager.
     *
     * <p>No model cache is kept here: a resource reload replaces the manager's
     * baked instances and the next lookup observes the new instance. Unknown,
     * incomplete, and missing-resource entries all return empty and log a
     * diagnostic instead of borrowing a random model.</p>
     */
    public static Optional<ModelHandle> lookup(String templateId) {
        Optional<ModelDefinition> definition = definition(templateId);
        if (definition.isEmpty()) {
            LOGGER.warn("No Bong model definition for template_id={}", templateId);
            return Optional.empty();
        }

        ModelDefinition modelDefinition = definition.orElseThrow();
        if (modelDefinition.modelId().isEmpty()) {
            LOGGER.warn(
                "Bong model definition for template_id={} is an explicit borrow candidate from {} "
                    + "and is not renderable until it owns a model definition and transforms",
                templateId,
                modelDefinition.borrowsFrom().orElse("<missing target>")
            );
            return Optional.empty();
        }

        MinecraftClient client = MinecraftClient.getInstance();
        BakedModelManager manager = client.getBakedModelManager();
        ModelIdentifier modelId = modelDefinition.modelId().orElseThrow();
        BakedModel model = manager.getModel(modelId);
        if (model == manager.getMissingModel()) {
            LOGGER.warn("Bong model resource is missing for template_id={}, model={}", templateId, modelId);
            return Optional.empty();
        }

        return Optional.of(new ModelHandle(
            modelDefinition.templateId(),
            modelId,
            model,
            model.getTransformation()
        ));
    }

    private static ModelIdentifier modelId(String path) {
        return new ModelIdentifier(new Identifier("bong", path), "inventory");
    }

    /** Canonical model metadata used by the P0 spike and future active plan. */
    public record ModelDefinition(
        String templateId,
        Optional<ModelIdentifier> modelId,
        Optional<String> borrowsFrom
    ) {
    }

    /** Baked model plus the transform block owned by that Bong model definition. */
    public record ModelHandle(
        String templateId,
        ModelIdentifier modelId,
        BakedModel model,
        ModelTransformation transformation
    ) {
    }
}
