package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongAnimationAssetManifestTest {
    private static final Path RESOURCE_ROOT =
        Path.of("src/main/resources/assets/bong/player_animation");

    private static final List<String> REQUIRED_IMPLEMENTATION_V1_ASSETS = List.of(
        "sword_swing_right",
        "meditate_sit",
        "hurt_stagger",
        "fist_punch_right",
        "fist_punch_left",
        "palm_strike",
        "sword_slash_down",
        "windup_charge",
        "release_burst",
        "parry_block",
        "dodge_roll",
        "harvest_crouch",
        "loot_bend",
        "stealth_crouch",
        "idle_breathe",
        "npc_patrol_walk",
        "npc_chop_tree",
        "npc_mine",
        "npc_crouch_wave",
        "npc_flee_run",
        "forge_hammer",
        "alchemy_stir",
        "lingtian_till",
        "inventory_reach",
        "stance_baomai",
        "stance_dugu",
        "stance_zhenfa",
        "stance_dugu_poison",
        "stance_zhenmai",
        "stance_woliu",
        "stance_tuike",
        "limp_left",
        "limp_right",
        "arm_injured_left",
        "arm_injured_right",
        "exhausted_walk",
        "breakthrough_yinqi",
        "breakthrough_ningmai",
        "breakthrough_guyuan",
        "breakthrough_tongling",
        "death_collapse",
        "death_disintegrate",
        "rebirth_wake"
    );

    @Test
    void implementationV1ProvidesAllPromisedAnimationAssets() throws IOException {
        assertTrue(REQUIRED_IMPLEMENTATION_V1_ASSETS.size() >= 25);
        for (String id : REQUIRED_IMPLEMENTATION_V1_ASSETS) {
            Path path = RESOURCE_ROOT.resolve(id + ".json");
            assertTrue(Files.isRegularFile(path), "缺少动画资源: " + path);
            assertValidPlayerAnimationJson(id, path);
        }
    }

    @Test
    void centralRollAnimationsResetAtBoundaries() throws IOException {
        for (String id : REQUIRED_IMPLEMENTATION_V1_ASSETS) {
            Path path = RESOURCE_ROOT.resolve(id + ".json");
            JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
            JsonObject emote = root.getAsJsonObject("emote");
            JsonArray moves = emote.getAsJsonArray("moves");
            int endTick = emote.get("endTick").getAsInt();

            assertBoundaryRollResetIfUsed(id, moves, endTick, "head");
            assertBoundaryRollResetIfUsed(id, moves, endTick, "torso");
        }
    }

    private static void assertValidPlayerAnimationJson(String id, Path path) throws IOException {
        JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
        assertEquals(3, root.get("version").getAsInt(), id + " 必须是 Emotecraft v3 JSON");
        assertEquals(id, root.get("name").getAsString(), id + " 文件名必须和 JSON name 一致");
        JsonObject emote = root.getAsJsonObject("emote");
        assertTrue(emote.get("endTick").getAsInt() > 0, id + " endTick 必须为正");
        assertFalse(emote.get("degrees").getAsBoolean(), id + " 运行时 JSON 应使用弧度");
        JsonArray moves = emote.getAsJsonArray("moves");
        assertTrue(moves.size() > 0, id + " 必须含关键帧");
    }

    private static void assertBoundaryRollResetIfUsed(
        String id,
        JsonArray moves,
        int endTick,
        String part
    ) {
        Map<Integer, Double> rollByTick = new HashMap<>();
        for (int i = 0; i < moves.size(); i++) {
            JsonObject move = moves.get(i).getAsJsonObject();
            JsonObject axes = move.getAsJsonObject(part);
            if (axes != null && axes.has("roll")) {
                rollByTick.put(move.get("tick").getAsInt(), axes.get("roll").getAsDouble());
            }
        }
        if (rollByTick.isEmpty()) {
            return;
        }
        assertEquals(0.0, rollByTick.getOrDefault(0, Double.NaN), 1e-7,
            id + " 使用 " + part + ".roll 时必须在 tick 0 复位");
        assertEquals(0.0, rollByTick.getOrDefault(endTick, Double.NaN), 1e-7,
            id + " 使用 " + part + ".roll 时必须在 endTick 复位");
    }
}
