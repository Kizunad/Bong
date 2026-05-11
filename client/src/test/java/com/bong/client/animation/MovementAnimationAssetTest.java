package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementAnimationAssetTest {
    @Test
    void movementV1ProvidesAllAnimationAssets() throws IOException {
        for (Identifier id : BongAnimations.MOVEMENT_V1_ANIMATIONS) {
            String resource = "/assets/bong/player_animation/" + id.getPath() + ".json";
            var input = MovementAnimationAssetTest.class.getResourceAsStream(resource);
            assertTrue(input != null, "missing movement animation asset: " + resource);
            JsonObject root;
            try (input; var reader = new InputStreamReader(input, StandardCharsets.UTF_8)) {
                root = JsonParser.parseReader(reader).getAsJsonObject();
            }
            assertEquals(3, root.get("version").getAsInt());
            assertEquals(id.getPath(), root.get("name").getAsString());
            JsonObject emote = root.getAsJsonObject("emote");
            assertTrue(emote.get("endTick").getAsInt() > 0);
            assertTrue(!emote.get("degrees").getAsBoolean());
            JsonArray moves = emote.getAsJsonArray("moves");
            assertTrue(moves.size() > 0);
        }
    }
}
