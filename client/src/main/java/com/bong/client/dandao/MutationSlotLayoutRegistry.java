package com.bong.client.dandao;

import com.bong.client.BongClient;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * plan-race-system-v1 P0 review r3 -- loads the shared
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} resource at startup and
 * caches it. Mirrors {@code ZoneAtmosphereProfileRegistry}'s classpath-load +
 * hardcoded-fallback shape (established precedent, see
 * {@code com.bong.client.atmosphere}).
 *
 * <p>{@link #fallbackDefaults()} exists purely as a defensive measure for a
 * missing/corrupt classpath resource at runtime -- it is kept byte-for-byte in sync
 * with the checked-in JSON resource by
 * {@code MutationSlotLayoutRegistryTest#fallbackDefaultsMatchClasspathResource}; the
 * checked-in resource (in turn pinned against {@code humanoid.json} on the server side,
 * see {@code body_plan::registry::humanoid_plan_static_mutation_slot_mapping_matches_client_shared_resource})
 * is the actual source of truth, not this fallback.
 */
public final class MutationSlotLayoutRegistry {
    static final String RESOURCE_PATH = "assets/bong/body_plans/humanoid_mutation_slots.json";

    private static volatile MutationSlotLayout instance;

    private MutationSlotLayoutRegistry() {}

    public static MutationSlotLayout get() {
        MutationSlotLayout loaded = instance;
        if (loaded == null) {
            synchronized (MutationSlotLayoutRegistry.class) {
                loaded = instance;
                if (loaded == null) {
                    loaded = loadDefault();
                    instance = loaded;
                }
            }
        }
        return loaded;
    }

    /** Test-only hook: forces the next {@link #get()} to reload from classpath/fallback. */
    static void resetForTest() {
        instance = null;
    }

    static MutationSlotLayout loadDefault() {
        MutationSlotLayout fromClasspath = loadClasspathLayout();
        return fromClasspath != null ? fromClasspath : fallbackDefaults();
    }

    private static MutationSlotLayout loadClasspathLayout() {
        try (InputStream stream =
                 MutationSlotLayoutRegistry.class.getClassLoader().getResourceAsStream(RESOURCE_PATH)) {
            if (stream == null) {
                return null;
            }
            String json = new String(stream.readAllBytes(), StandardCharsets.UTF_8);
            MutationSlotLayoutParser.ParseResult result = MutationSlotLayoutParser.parse(json);
            if (result.ok()) {
                return result.layout();
            }
            BongClient.LOGGER.warn("Ignoring mutation slot layout {}: {}", RESOURCE_PATH, result.error());
            return null;
        } catch (IOException ex) {
            BongClient.LOGGER.warn("Failed to load mutation slot layout {}: {}", RESOURCE_PATH, ex.toString());
            return null;
        }
    }

    static MutationSlotLayout fallbackDefaults() {
        Map<String, MutationSlotLayout.SlotEntry> slots = new LinkedHashMap<>();
        slots.put("Head", new MutationSlotLayout.SlotEntry(
            "head", new MutationSlotLayout.Anchor(0.0f, 1.65f, 0.0f, 1.031f)));
        slots.put("Forearm", new MutationSlotLayout.SlotEntry(
            "arm_r", new MutationSlotLayout.Anchor(0.32f, 1.2f, 0.0f, 1.032f)));
        slots.put("Back", new MutationSlotLayout.SlotEntry(
            "back", new MutationSlotLayout.Anchor(0.0f, 1.2f, -0.15f, 1.033f)));
        slots.put("Torso", new MutationSlotLayout.SlotEntry(
            "chest", new MutationSlotLayout.Anchor(0.0f, 1.2f, 0.1f, 1.034f)));
        slots.put("Lower", new MutationSlotLayout.SlotEntry(
            "abdomen", new MutationSlotLayout.Anchor(0.0f, 0.95f, -0.05f, 1.035f)));
        return new MutationSlotLayout("humanoid", slots);
    }
}
