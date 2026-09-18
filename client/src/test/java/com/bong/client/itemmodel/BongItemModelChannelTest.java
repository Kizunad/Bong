package com.bong.client.itemmodel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

import org.junit.jupiter.api.Test;

class BongItemModelChannelTest {
    @Test
    void ownModelUsesBongNamespaceAndInventoryVariant() {
        BongItemModelChannel.ModelDefinition definition = BongItemModelChannel.definition("wooden_shield")
            .orElseThrow(() -> new AssertionError("P0 own model definition missing"));

        assertEquals("wooden_shield", definition.templateId());
        assertTrue(definition.borrowsFrom().isEmpty());
        assertEquals("bong", definition.modelId().orElseThrow().getNamespace());
        assertEquals("item/wooden_shield/wooden_shield", definition.modelId().orElseThrow().getPath());
        assertEquals("inventory", definition.modelId().orElseThrow().getVariant());
    }

    @Test
    void borrowCandidateIsExplicitAndFailsClosedUntilItOwnsTransforms() {
        BongItemModelChannel.ModelDefinition definition = BongItemModelChannel.definition("qing_feng_sword")
            .orElseThrow(() -> new AssertionError("borrow candidate missing"));

        assertTrue(definition.modelId().isEmpty(), "candidate must not silently load a target model");
        assertEquals("iron_sword", definition.borrowsFrom().orElseThrow());
    }

    @Test
    void ownModelResourceContainsItsOwnTransformBlock() throws IOException {
        String resourcePath = "/assets/bong/models/item/wooden_shield/wooden_shield.json";
        try (InputStream stream = getClass().getResourceAsStream(resourcePath)) {
            assertTrue(stream != null, "missing Bong-owned model resource: " + resourcePath);
            String json = new String(stream.readAllBytes(), StandardCharsets.UTF_8);
            assertTrue(json.contains("sml:builtin/obj"));
            assertTrue(json.contains("bong:models/item/wooden_shield/wooden_shield.obj"));
            assertTrue(json.contains("thirdperson_righthand"));
            assertTrue(json.contains("thirdperson_lefthand"));
            assertTrue(json.contains("firstperson_righthand"));
            assertTrue(json.contains("firstperson_lefthand"));
            assertTrue(json.contains("\"ground\""));
            assertTrue(json.contains("\"gui\""));
            assertTrue(json.contains("\"fixed\""));
            assertFalse(json.contains("minecraft:item/"), "model must not route through a vanilla host");
        }
    }
}
