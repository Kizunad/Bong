package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
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
        Map<String, Double> tick0 = axisValuesAtTick(moves, 0);
        Map<String, Double> end = axisValuesAtTick(moves, endTick);
        // Check all tick-0 axes are present at endTick (guards against single-frame decay)
        assertTrue(end.keySet().containsAll(tick0.keySet()),
            id + " is looped, so every tick-0 axis must also be keyed at endTick to avoid single-frame decay; missing "
                + difference(tick0.keySet(), end.keySet()));
        // Check values are identical per axis (guards against visible playback jump at loop boundary)
        for (String axis : tick0.keySet()) {
            assertEquals(tick0.get(axis), end.get(axis),
                id + " loop boundary value mismatch for axis=" + axis
                    + ": tick0=" + tick0.get(axis) + " endTick=" + end.get(axis)
                    + " — PlayerAnimator decays looped single-keyframe axes to defaultValue, "
                    + "so首尾值必须一致否则会出现回环跳变");
        }
    }

    /**
     * Returns a map from "part.axis" to the (first) numeric value at {@code tick}.
     * Non-numeric values are skipped (e.g. string easing fields nested inside part objects).
     */
    private static Map<String, Double> axisValuesAtTick(JsonArray moves, int tick) {
        Map<String, Double> out = new HashMap<>();
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
                    var elem = partAxes.get(axis);
                    if (elem.isJsonPrimitive() && elem.getAsJsonPrimitive().isNumber()) {
                        out.putIfAbsent(part + "." + axis, elem.getAsDouble());
                    }
                }
            }
        }
        return out;
    }

    private static Set<String> axesAtTick(JsonArray moves, int tick) {
        return axisValuesAtTick(moves, tick).keySet();
    }

    private static Set<String> difference(Set<String> left, Set<String> right) {
        Set<String> out = new HashSet<>(left);
        out.removeAll(right);
        return out;
    }
}
