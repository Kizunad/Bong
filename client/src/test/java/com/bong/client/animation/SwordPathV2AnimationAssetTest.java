package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SwordPathV2AnimationAssetTest {
    @Test
    void swordPathV2ProvidesAllPromisedAnimationAssets() throws IOException {
        assertEquals(6, BongAnimations.SWORD_PATH_V2_ANIMATIONS.size());

        Set<String> seen = new HashSet<>();
        for (Identifier id : BongAnimations.SWORD_PATH_V2_ANIMATIONS) {
            assertTrue(seen.add(id.getPath()), "duplicate sword-path-v2 animation id: " + id);
            String resource = "/assets/bong/player_animation/" + id.getPath() + ".json";
            JsonObject root = readResource(resource);
            assertEquals(3, root.get("version").getAsInt(), id + " must use Emotecraft v3 JSON");
            assertEquals(id.getPath(), root.get("name").getAsString());

            JsonObject emote = root.getAsJsonObject("emote");
            assertNotNull(emote, id + " must define emote");
            assertTrue(emote.get("endTick").getAsInt() > 0, id + " endTick must be positive");
            assertTrue(emote.get("stopTick").getAsInt() >= emote.get("endTick").getAsInt() + 2,
                id + " stopTick should leave enough return time for PlayerAnimator cleanup");
            assertFalse(emote.get("degrees").getAsBoolean(), id + " must use radians");
            JsonArray moves = emote.getAsJsonArray("moves");
            assertTrue(moves.size() > 0, id + " must contain keyframes");

            if (emote.get("isLoop").getAsBoolean()) {
                assertLoopedAxesHaveEndTickKeys(id.getPath(), emote);
            }
        }
    }

    private static JsonObject readResource(String resource) throws IOException {
        var input = SwordPathV2AnimationAssetTest.class.getResourceAsStream(resource);
        assertNotNull(input, "missing sword-path-v2 animation asset: " + resource);
        try (input; var reader = new InputStreamReader(input, StandardCharsets.UTF_8)) {
            return JsonParser.parseReader(reader).getAsJsonObject();
        }
    }

    private static void assertLoopedAxesHaveEndTickKeys(String id, JsonObject emote) {
        int endTick = emote.get("endTick").getAsInt();
        JsonArray moves = emote.getAsJsonArray("moves");
        Set<String> tick0Axes = axesAtTick(moves, 0);
        Set<String> endAxes = axesAtTick(moves, endTick);
        assertTrue(endAxes.containsAll(tick0Axes),
            id + " is looped, so every tick-0 axis must also be keyed at endTick to avoid single-frame decay; missing "
                + difference(tick0Axes, endAxes));
    }

    private static Set<String> axesAtTick(JsonArray moves, int tick) {
        Set<String> axes = new HashSet<>();
        for (int i = 0; i < moves.size(); i++) {
            JsonObject move = moves.get(i).getAsJsonObject();
            if (move.get("tick").getAsInt() != tick) {
                continue;
            }
            for (String part : move.keySet()) {
                if ("tick".equals(part) || "easing".equals(part)) {
                    continue;
                }
                JsonObject partAxes = move.getAsJsonObject(part);
                for (String axis : partAxes.keySet()) {
                    axes.add(part + "." + axis);
                }
            }
        }
        return axes;
    }

    private static Set<String> difference(Set<String> left, Set<String> right) {
        Set<String> out = new HashSet<>(left);
        out.removeAll(right);
        return out;
    }
}
