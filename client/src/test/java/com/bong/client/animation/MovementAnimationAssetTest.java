package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementAnimationAssetTest {
    private static final Path RESOURCE_ROOT =
        Path.of("src/main/resources/assets/bong/player_animation");

    @Test
    void movementV1ProvidesAllAnimationAssets() throws IOException {
        for (Identifier id : BongAnimations.MOVEMENT_V1_ANIMATIONS) {
            Path path = RESOURCE_ROOT.resolve(id.getPath() + ".json");
            assertTrue(Files.isRegularFile(path), "missing movement animation asset: " + path);
            JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
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
