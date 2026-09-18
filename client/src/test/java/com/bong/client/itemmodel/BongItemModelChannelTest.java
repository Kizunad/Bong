package com.bong.client.itemmodel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Set;

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
            JsonObject model = JsonParser.parseString(
                new String(stream.readAllBytes(), StandardCharsets.UTF_8)
            ).getAsJsonObject();

            assertEquals("sml:builtin/obj", model.get("parent").getAsString());
            assertEquals(
                "bong:models/item/wooden_shield/wooden_shield.obj",
                model.get("model").getAsString()
            );
            assertTrue(
                model.has("display") && model.get("display").isJsonObject(),
                "model display must be a JSON object"
            );
            JsonObject display = model.getAsJsonObject("display");
            assertEquals(
                Set.of(
                    "thirdperson_righthand",
                    "thirdperson_lefthand",
                    "firstperson_righthand",
                    "firstperson_lefthand",
                    "ground",
                    "gui",
                    "fixed"
                ),
                display.keySet(),
                "model display must declare all seven transform contexts"
            );
            assertNoVanillaItemModelReferences(model, "$");
        }
    }

    private static void assertNoVanillaItemModelReferences(JsonElement element, String path) {
        if (element.isJsonPrimitive() && element.getAsJsonPrimitive().isString()) {
            assertFalse(
                element.getAsString().startsWith("minecraft:item/"),
                () -> "model value must not route through a vanilla host at " + path
            );
            return;
        }
        if (element.isJsonArray()) {
            for (int index = 0; index < element.getAsJsonArray().size(); index++) {
                assertNoVanillaItemModelReferences(element.getAsJsonArray().get(index), path + "[" + index + "]");
            }
            return;
        }
        if (element.isJsonObject()) {
            for (var entry : element.getAsJsonObject().entrySet()) {
                assertNoVanillaItemModelReferences(entry.getValue(), path + "." + entry.getKey());
            }
        }
    }
}
