package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementAnimationAssetTest {
    private static final Set<String> LOWER_BODY_PARTS = Set.of("rightLeg", "leftLeg", "body");
    private static final Set<String> LOWER_BODY_BODY_AXES = Set.of("pitch", "y", "z");
    private static final Set<String> JIAN_PARTS = Set.of("rightArm", "leftArm", "torso", "head", "body");
    private static final Set<String> JIAN_BODY_AXES = Set.of("y", "z");

    @Test
    void movementV1ProvidesAllAnimationAssets() throws IOException {
        for (Identifier id : BongAnimations.MOVEMENT_V1_ANIMATIONS) {
            JsonObject root = readAsset(id);
            assertEquals(3, root.get("version").getAsInt(), id + " 必须是 Emotecraft v3");
            assertEquals(id.getPath(), root.get("name").getAsString(), id + " name 必须与文件名一致");
            JsonObject emote = root.getAsJsonObject("emote");
            assertTrue(emote.get("endTick").getAsInt() > 0, id + " endTick 必须为正");
            assertFalse(emote.get("degrees").getAsBoolean(), id + " 运行时资产必须使用弧度");
            assertTrue(emote.getAsJsonArray("moves").size() > 0, id + " 必须含关键帧");
        }
    }

    @Test
    void lowerBodyGaitsBindToProductionAssetsAndNeverWriteUpperBodyParts() throws IOException {
        for (Identifier id : BongAnimations.LOWER_BODY_GAIT_ANIMATIONS) {
            JsonObject root = readAsset(id);
            JsonObject emote = root.getAsJsonObject("emote");
            Map<String, Map<String, Map<Integer, Double>>> tracks = tracks(emote);
            for (Map.Entry<String, Map<String, Map<Integer, Double>>> part : tracks.entrySet()) {
                assertTrue(LOWER_BODY_PARTS.contains(part.getKey()),
                    id + " 只能写下半身部件，实际写了 " + part.getKey());
                if (part.getKey().equals("body")) {
                    assertTrue(LOWER_BODY_BODY_AXES.containsAll(part.getValue().keySet()),
                        id + " body 只允许 pitch/y/z，实际 " + part.getValue().keySet());
                }
            }
            assertBoundaryTracks(id, emote, tracks, emote.get("isLoop").getAsBoolean());
            if (id.equals(BongAnimations.LOWER_DASH)) {
                assertTerminalReset(id, tracks);
            }
        }
    }

    @Test
    void jianAssetsBindToProductionAssetsAndNeverWriteLowerBodyParts() throws IOException {
        for (Identifier id : BongAnimations.JIAN_ANIMATIONS) {
            JsonObject root = readAsset(id);
            JsonObject emote = root.getAsJsonObject("emote");
            Map<String, Map<String, Map<Integer, Double>>> tracks = tracks(emote);
            for (Map.Entry<String, Map<String, Map<Integer, Double>>> part : tracks.entrySet()) {
                assertTrue(JIAN_PARTS.contains(part.getKey()),
                    id + " 只能写上半身部件，实际写了 " + part.getKey());
                if (part.getKey().equals("body")) {
                    assertTrue(JIAN_BODY_AXES.containsAll(part.getValue().keySet()),
                        id + " body 只允许 y/z，实际 " + part.getValue().keySet());
                }
            }
            assertBoundaryTracks(id, emote, tracks, emote.get("isLoop").getAsBoolean());
        }
    }

    private static JsonObject readAsset(Identifier id) throws IOException {
        String resource = "/assets/bong/player_animation/" + id.getPath() + ".json";
        var input = MovementAnimationAssetTest.class.getResourceAsStream(resource);
        assertTrue(input != null, "missing production animation asset: " + resource);
        try (input; var reader = new InputStreamReader(input, StandardCharsets.UTF_8)) {
            return JsonParser.parseReader(reader).getAsJsonObject();
        }
    }

    private static Map<String, Map<String, Map<Integer, Double>>> tracks(JsonObject emote) {
        Map<String, Map<String, Map<Integer, Double>>> result = new HashMap<>();
        JsonArray moves = emote.getAsJsonArray("moves");
        for (JsonElement element : moves) {
            JsonObject move = element.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            for (Map.Entry<String, JsonElement> part : move.entrySet()) {
                if (part.getKey().equals("tick") || part.getKey().equals("easing")) {
                    continue;
                }
                for (Map.Entry<String, JsonElement> axis : part.getValue().getAsJsonObject().entrySet()) {
                    result.computeIfAbsent(part.getKey(), unused -> new HashMap<>())
                        .computeIfAbsent(axis.getKey(), unused -> new HashMap<>())
                        .put(tick, axis.getValue().getAsDouble());
                }
            }
        }
        return result;
    }

    private static void assertBoundaryTracks(
        Identifier id,
        JsonObject emote,
        Map<String, Map<String, Map<Integer, Double>>> tracks,
        boolean requireEqual
    ) {
        int endTick = emote.get("endTick").getAsInt();
        for (Map.Entry<String, Map<String, Map<Integer, Double>>> part : tracks.entrySet()) {
            for (Map.Entry<String, Map<Integer, Double>> axis : part.getValue().entrySet()) {
                assertTrue(axis.getValue().containsKey(0),
                    id + " " + part.getKey() + "." + axis.getKey() + " 缺 tick 0");
                assertTrue(axis.getValue().containsKey(endTick),
                    id + " " + part.getKey() + "." + axis.getKey() + " 缺 endTick");
                if (requireEqual) {
                    assertEquals(axis.getValue().get(0), axis.getValue().get(endTick), 1e-6,
                        id + " 循环轨道必须在 endTick 闭合：" + part.getKey() + "." + axis.getKey());
                }
            }
        }
    }

    private static void assertTerminalReset(
        Identifier id,
        Map<String, Map<String, Map<Integer, Double>>> tracks
    ) {
        for (Map.Entry<String, Map<String, Map<Integer, Double>>> part : tracks.entrySet()) {
            for (Map.Entry<String, Map<Integer, Double>> axis : part.getValue().entrySet()) {
                assertEquals(0.0, axis.getValue().get(axis.getValue().keySet().stream().max(Integer::compareTo).orElseThrow()), 1e-6,
                    id + " 一次性步态末帧必须归零：" + part.getKey() + "." + axis.getKey());
            }
        }
    }
}
