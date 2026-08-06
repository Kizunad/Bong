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
import java.util.HashSet;
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
    void lowerDashEndsAtServerBoundaryWithoutResidualPose() throws IOException {
        JsonObject emote = readAsset(BongAnimations.LOWER_DASH).getAsJsonObject("emote");
        Map<String, Map<String, Map<Integer, Double>>> tracks = tracks(emote);
        int serverDashDurationTicks = 4;

        assertFalse(emote.get("isLoop").getAsBoolean(), "DASH 必须是一次性动画");
        assertEquals(serverDashDurationTicks, emote.get("endTick").getAsInt(),
            "DASH 可见时间线必须与服务端四 tick 窗口一致");
        assertEquals(serverDashDurationTicks, emote.get("stopTick").getAsInt(),
            "DASH stopTick 不得延伸到服务端动作结束之后");

        for (Map.Entry<String, Map<String, Map<Integer, Double>>> part : tracks.entrySet()) {
            for (Map.Entry<String, Map<Integer, Double>> axis : part.getValue().entrySet()) {
                assertTrue(axis.getValue().keySet().stream().allMatch(tick -> tick <= serverDashDurationTicks),
                    "DASH 在 tick 4 后不得残留关键帧：" + part.getKey() + "." + axis.getKey());
                assertEquals(0.0, axis.getValue().get(serverDashDurationTicks), 1e-6,
                    "DASH tick 4 必须归零，避免服务端结束后残留姿态："
                        + part.getKey() + "." + axis.getKey());
            }
        }
    }

    @Test
    void jianAssetsBindToProductionAssetsAndNeverWriteLowerBodyParts() throws IOException {
        for (Identifier id : BongAnimations.JIAN_ANIMATIONS) {
            JsonObject root = readAsset(id);
            JsonObject emote = root.getAsJsonObject("emote");
            assertJianMetadata(id, root, emote);
            assertTrue(emote.getAsJsonArray("moves").size() > 0, id + " 必须含关键帧");
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
            if (id.equals(BongAnimations.JIAN_WAIST_SPIN_CROSS)) {
                assertEquals("contract-first-unwired", root.get("wiring").getAsString(),
                    id + " 必须明确声明 contract-first-unwired，避免无生产 producer 的半接线资产");
            }
        }
    }

    private static void assertJianMetadata(Identifier id, JsonObject root, JsonObject emote) {
        assertEquals(3, root.get("version").getAsInt(), id + " 必须是 Emotecraft v3");
        assertEquals("Bong", root.get("author").getAsString(), id + " author 必须保持资产归属");
        assertEquals(id.getPath(), root.get("name").getAsString(), id + " name 必须与文件名一致");
        assertEquals(0, emote.get("beginTick").getAsInt(), id + " beginTick 必须从 0 开始");
        assertEquals(0, emote.get("returnTick").getAsInt(), id + " returnTick 必须保持为 0");
        assertFalse(emote.get("nsfw").getAsBoolean(), id + " 运行时资产不得标记为 NSFW");
        assertFalse(emote.get("degrees").getAsBoolean(), id + " 运行时资产必须使用弧度");

        int expectedEndTick;
        int expectedStopTick;
        boolean expectedLoop;
        if (id.equals(BongAnimations.JIAN_DRAW_WAIST)) {
            expectedEndTick = 24;
            expectedStopTick = 27;
            expectedLoop = false;
        } else if (id.equals(BongAnimations.JIAN_DUAL_SMASH)) {
            expectedEndTick = 18;
            expectedStopTick = 21;
            expectedLoop = false;
        } else if (id.equals(BongAnimations.JIAN_DUAL_SWEEP)) {
            expectedEndTick = 22;
            expectedStopTick = 25;
            expectedLoop = false;
        } else if (id.equals(BongAnimations.JIAN_STANCE_HIGH_LOW)) {
            expectedEndTick = 40;
            expectedStopTick = 43;
            expectedLoop = true;
        } else if (id.equals(BongAnimations.JIAN_WAIST_SPIN_CROSS)) {
            expectedEndTick = 32;
            expectedStopTick = 35;
            expectedLoop = false;
        } else {
            throw new AssertionError("未声明的双锏资产元数据契约：" + id);
        }
        assertEquals(expectedEndTick, emote.get("endTick").getAsInt(), id + " endTick 不能漂移");
        assertEquals(expectedStopTick, emote.get("stopTick").getAsInt(), id + " stopTick 不能漂移");
        assertEquals(expectedLoop, emote.get("isLoop").getAsBoolean(), id + " isLoop 不能漂移");
    }

    @Test
    void jianLoopAndStopContractsRemainPinned() throws IOException {
        assertJianLoopContract(BongAnimations.JIAN_STANCE_HIGH_LOW, true, 40, 43);
        assertJianLoopContract(BongAnimations.JIAN_DUAL_SWEEP, false, 22, 25);
    }

    private static void assertJianLoopContract(
        Identifier id,
        boolean expectedLoop,
        int expectedEndTick,
        int expectedStopTick
    ) throws IOException {
        JsonObject emote = readAsset(id).getAsJsonObject("emote");
        assertEquals(expectedLoop, emote.get("isLoop").getAsBoolean(), id + " isLoop 契约不能由资产自身推导");
        assertEquals(expectedEndTick, emote.get("endTick").getAsInt(), id + " endTick 契约不能漂移");
        assertEquals(expectedStopTick, emote.get("stopTick").getAsInt(), id + " stopTick 契约不能漂移");
    }

    @Test
    void jianActionTimelinesRetainAuthoredPhases() throws IOException {
        assertActionTimeline(
            BongAnimations.JIAN_DRAW_WAIST,
            Set.of(0, 6, 9, 14, 17, 24),
            Map.of(0, "INOUTSINE", 6, "INOUTSINE", 9, "INOUTSINE",
                14, "OUTQUAD", 17, "OUTQUAD", 24, "INOUTSINE"),
            Map.of(
                "rightArm.pitch", Map.of(6, -0.5235988, 14, -1.5358897, 17, -1.8151424),
                "torso.pitch", Map.of(6, 0.100531, 14, -0.1256637, 17, -0.1759292),
                "body.y", Map.of(6, 0.05, 14, -0.05, 17, -0.03)
            )
        );
        assertActionTimeline(
            BongAnimations.JIAN_DUAL_SMASH,
            Set.of(0, 6, 8, 11, 13, 15, 18),
            Map.of(0, "INOUTSINE", 6, "INOUTSINE", 8, "INOUTSINE",
                11, "OUTQUAD", 13, "OUTQUAD", 15, "INOUTSINE", 18, "INOUTSINE"),
            Map.of(
                "rightArm.bend", Map.of(6, 0.3839724, 8, 0.3141593, 11, 0.0872665, 15, 0.3141593),
                "torso.pitch", Map.of(6, -0.2261947, 8, -0.2638938, 11, 0.3267256, 13, 0.4021239),
                "body.z", Map.of(6, -0.05, 8, -0.06, 11, 0.14, 13, 0.1)
            )
        );
    }

    private static void assertActionTimeline(
        Identifier id,
        Set<Integer> expectedTicks,
        Map<Integer, String> expectedEasing,
        Map<String, Map<Integer, Double>> expectedValues
    ) throws IOException {
        JsonObject emote = readAsset(id).getAsJsonObject("emote");
        JsonArray moves = emote.getAsJsonArray("moves");
        Set<Integer> actualTicks = new HashSet<>();
        Map<Integer, String> actualEasing = new HashMap<>();
        Map<String, Map<Integer, Double>> actualValues = new HashMap<>();
        for (JsonElement element : moves) {
            JsonObject move = element.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            actualTicks.add(tick);
            actualEasing.putIfAbsent(tick, move.get("easing").getAsString());
            for (Map.Entry<String, Map<Integer, Double>> expectedTrack : expectedValues.entrySet()) {
                String[] path = expectedTrack.getKey().split("\\.");
                if (expectedTrack.getValue().containsKey(tick)
                    && move.has(path[0]) && move.getAsJsonObject(path[0]).has(path[1])) {
                    actualValues.computeIfAbsent(expectedTrack.getKey(), unused -> new HashMap<>())
                        .put(tick, move.getAsJsonObject(path[0]).get(path[1]).getAsDouble());
                }
            }
        }
        assertEquals(expectedTicks, actualTicks, id + " authored timeline ticks must not disappear");
        assertEquals(expectedEasing, actualEasing, id + " authored easing phases must remain intact");
        for (Map.Entry<String, Map<Integer, Double>> expectedTrack : expectedValues.entrySet()) {
            assertEquals(expectedTrack.getValue(), actualValues.get(expectedTrack.getKey()),
                id + " representative pose track must retain authored values: " + expectedTrack.getKey());
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
