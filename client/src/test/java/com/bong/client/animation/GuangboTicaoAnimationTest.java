package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * #4 各招专属动画：广播体操完整套路资产校验。
 *
 * <p>此前 server `ANIM_GUANGBO_TICAO` 复用 `guard_raise`（4tick 举臂格挡），真机"只动一下"。
 * 现改发专属 `bong:guangbo_ticao`（client `BongAnimationRegistry` 自动扫描注册）。本测试锁住
 * 该资产是**完整多节套路**（非短 stub），结构合法且 roll 在边界复位（防体侧节侧屈残留漂移）。
 */
class GuangboTicaoAnimationTest {
    private static final Path JSON =
        Path.of("src/main/resources/assets/bong/player_animation/guangbo_ticao.json");

    @Test
    void assetExistsAndIsValidEmotecraftV3() throws IOException {
        assertTrue(Files.isRegularFile(JSON), "缺少广播体操动画资源: " + JSON);
        JsonObject root = JsonParser.parseString(Files.readString(JSON)).getAsJsonObject();
        assertEquals(3, root.get("version").getAsInt(), "必须是 Emotecraft v3 JSON");
        assertEquals("guangbo_ticao", root.get("name").getAsString(), "JSON name 必须与文件名一致");
        JsonObject emote = root.getAsJsonObject("emote");
        assertFalse(emote.get("degrees").getAsBoolean(), "运行时 JSON 应使用弧度");
    }

    @Test
    void isFullRoutineNotBriefStub() throws IOException {
        // 锁住"完整动画"：endTick 远大于此前复用的 guard_raise(4tick)，且关键帧足够多（5 节套路）。
        JsonObject emote = emote();
        int endTick = emote.get("endTick").getAsInt();
        assertTrue(endTick >= 120,
            "广播体操应为完整套路（endTick>=120tick/~6s），实际 " + endTick + "（避免回退成 guard_raise 的 4tick）");
        assertFalse(emote.get("isLoop").getAsBoolean(), "广播体操是一次性套路，非循环");
        int stopTick = emote.get("stopTick").getAsInt();
        assertTrue(stopTick >= endTick, "stopTick 必须 >= endTick 以缓收");
        assertTrue(emote.getAsJsonArray("moves").size() >= 60,
            "5 节套路应有充足关键帧（moves>=60），实际 " + emote.getAsJsonArray("moves").size());
    }

    @Test
    void torsoRollResetsAtBoundaries() throws IOException {
        // 体侧节用 torso.roll ±26° 侧屈；必须在 tick 0 与 endTick 复位 0，否则收势后躯干残留歪斜。
        JsonObject emote = emote();
        int endTick = emote.get("endTick").getAsInt();
        Map<Integer, Double> rollByTick = new HashMap<>();
        JsonArray moves = emote.getAsJsonArray("moves");
        for (int i = 0; i < moves.size(); i++) {
            JsonObject move = moves.get(i).getAsJsonObject();
            JsonObject torso = move.getAsJsonObject("torso");
            if (torso != null && torso.has("roll")) {
                rollByTick.put(move.get("tick").getAsInt(), torso.get("roll").getAsDouble());
            }
        }
        assertFalse(rollByTick.isEmpty(), "广播体操体侧节应使用 torso.roll");
        assertEquals(0.0, rollByTick.getOrDefault(0, Double.NaN), 1e-7, "torso.roll 必须在 tick 0 复位");
        assertEquals(0.0, rollByTick.getOrDefault(endTick, Double.NaN), 1e-7,
            "torso.roll 必须在 endTick 复位（收势归正，不残留侧屈）");
    }

    private static JsonObject emote() throws IOException {
        return JsonParser.parseString(Files.readString(JSON)).getAsJsonObject().getAsJsonObject("emote");
    }
}
