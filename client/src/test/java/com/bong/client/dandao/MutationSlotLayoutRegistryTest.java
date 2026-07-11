package com.bong.client.dandao;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-race-system-v1 P0 review r3 (major x3 收口) -- Tests for
 * {@link MutationSlotLayoutRegistry}. Covers loading the real checked-in
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} resource off the
 * classpath (mirrors {@code ZoneAtmosphereTest}'s established
 * {@code loadDefault()} testing pattern for this client) -- one dedicated
 * assertion per {@code BodySlot} variant, plus the unknown-slot and
 * fallback-drift-guard branches.
 */
class MutationSlotLayoutRegistryTest {

    @AfterEach
    void cleanup() {
        MutationSlotLayoutRegistry.resetForTest();
    }

    @Test
    void loadDefaultResolvesHeadSlotToHeadPart() {
        MutationSlotLayout.SlotEntry entry = MutationSlotLayoutRegistry.loadDefault().forBodySlot("Head");
        assertNotNull(entry, "BodySlot::Head must have a layout mapping");
        assertEquals("head", entry.partId());
    }

    @Test
    void loadDefaultResolvesForearmSlotToArmRPart() {
        MutationSlotLayout.SlotEntry entry = MutationSlotLayoutRegistry.loadDefault().forBodySlot("Forearm");
        assertNotNull(entry, "BodySlot::Forearm must have a layout mapping");
        assertEquals("arm_r", entry.partId());
    }

    @Test
    void loadDefaultResolvesBackSlotToBackPart() {
        MutationSlotLayout.SlotEntry entry = MutationSlotLayoutRegistry.loadDefault().forBodySlot("Back");
        assertNotNull(entry, "BodySlot::Back must have a layout mapping");
        assertEquals("back", entry.partId());
    }

    @Test
    void loadDefaultResolvesTorsoSlotToChestPart() {
        MutationSlotLayout.SlotEntry entry = MutationSlotLayoutRegistry.loadDefault().forBodySlot("Torso");
        assertNotNull(entry, "BodySlot::Torso must have a layout mapping");
        assertEquals("chest", entry.partId());
    }

    @Test
    void loadDefaultResolvesLowerSlotToAbdomenPart() {
        MutationSlotLayout.SlotEntry entry = MutationSlotLayoutRegistry.loadDefault().forBodySlot("Lower");
        assertNotNull(entry, "BodySlot::Lower must have a layout mapping");
        assertEquals("abdomen", entry.partId());
    }

    @Test
    void loadDefaultCoversExactlyTheFiveBodySlotVariants() {
        MutationSlotLayout layout = MutationSlotLayoutRegistry.loadDefault();
        assertEquals(5, layout.slots().size(),
            "shared resource must declare exactly the 5 BodySlot variants, not more/fewer");
    }

    @Test
    void unknownBodySlotReturnsNull() {
        MutationSlotLayout layout = MutationSlotLayoutRegistry.loadDefault();
        assertNull(layout.forBodySlot("Tentacle"),
            "a body_slot string with no mapping entry must resolve to null (explicit skip), not a guessed default");
    }

    @Test
    void getCachesInstanceAcrossCalls() {
        MutationSlotLayout first = MutationSlotLayoutRegistry.get();
        MutationSlotLayout second = MutationSlotLayoutRegistry.get();
        assertSame(first, second, "get() must cache the loaded layout rather than reparsing every call");
    }

    @Test
    void fallbackDefaultsMatchClasspathResource() {
        // Drift guard: the hardcoded Java fallback (only used if the classpath
        // resource is missing/corrupt at runtime) must stay byte-for-byte in sync
        // with the checked-in JSON resource that is actually loaded in practice.
        MutationSlotLayout fromClasspath = MutationSlotLayoutRegistry.loadDefault();
        MutationSlotLayout fallback = MutationSlotLayoutRegistry.fallbackDefaults();
        assertEquals(fromClasspath, fallback,
            "fallbackDefaults() has drifted from the checked-in humanoid_mutation_slots.json resource — "
                + "update one to match the other");
    }
}
