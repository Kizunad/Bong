package com.bong.client.preview;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;

/**
 * preview-harness.json 解析结果。字段缺省时使用默认值，screenshots[] 必填且
 * 至少 1 条。
 *
 * 期望 JSON 形态（详见 plan-worldgen-snapshot-v1.md §1.5）:
 * {
 *   "server": "127.0.0.1:25565",
 *   "username": "PreviewBot",
 *   "wait_world_ticks": 1200,
 *   "wait_chunks_ticks": 100,
 *   "settle_ticks": 20,
 *   "output_dir": "screenshots",
 *   "exit_on_complete": true,
 *   "screenshots": [
 *     { "name": "top", "tp": [8, 150, 8], "yaw": 0, "pitch": -90 }
 *   ]
 * }
 */
public record PreviewConfig(
        String server,
        String username,
        int waitWorldTicks,
        int waitChunksTicks,
        int settleTicks,
        String outputDir,
        boolean exitOnComplete,
        List<PreviewShot> screenshots
) {
    public PreviewConfig {
        if (screenshots == null || screenshots.isEmpty()) {
            throw new IllegalArgumentException("PreviewConfig.screenshots must contain at least one shot");
        }
        screenshots = List.copyOf(screenshots);
    }

    public static PreviewConfig load(Path path) throws IOException {
        String body = Files.readString(path);
        JsonObject root = JsonParser.parseString(body).getAsJsonObject();

        String server = optString(root, "server").orElse("127.0.0.1:25565");
        String username = optString(root, "username").orElse("PreviewBot");
        int waitWorldTicks = optInt(root, "wait_world_ticks").orElse(20 * 60);
        int waitChunksTicks = optInt(root, "wait_chunks_ticks").orElse(20 * 5);
        int settleTicks = optInt(root, "settle_ticks").orElse(20);
        String outputDir = optString(root, "output_dir").orElse("screenshots");
        boolean exitOnComplete = optBool(root, "exit_on_complete").orElse(true);

        if (!root.has("screenshots") || !root.get("screenshots").isJsonArray()) {
            throw new IllegalArgumentException("preview-harness.json missing required array field 'screenshots'");
        }
        JsonArray shotsJson = root.getAsJsonArray("screenshots");
        List<PreviewShot> shots = new ArrayList<>(shotsJson.size());
        for (int i = 0; i < shotsJson.size(); i++) {
            shots.add(parseShot(shotsJson.get(i).getAsJsonObject(), i));
        }
        if (shots.isEmpty()) {
            throw new IllegalArgumentException("preview-harness.json 'screenshots' array must not be empty");
        }

        return new PreviewConfig(
                server, username, waitWorldTicks, waitChunksTicks,
                settleTicks, outputDir, exitOnComplete,
                Collections.unmodifiableList(shots));
    }

    private static PreviewShot parseShot(JsonObject obj, int idx) {
        String name = optString(obj, "name")
                .orElseThrow(() -> new IllegalArgumentException(
                        "screenshots[" + idx + "].name 缺失"));
        if (!obj.has("tp") || !obj.get("tp").isJsonArray()) {
            throw new IllegalArgumentException(
                    "screenshots[" + idx + "].tp 必须是 [x, y, z] 数组");
        }
        JsonArray tpArr = obj.getAsJsonArray("tp");
        if (tpArr.size() != 3) {
            throw new IllegalArgumentException(
                    "screenshots[" + idx + "].tp 长度应为 3，实际 " + tpArr.size());
        }
        double[] tp = new double[]{
                tpArr.get(0).getAsDouble(),
                tpArr.get(1).getAsDouble(),
                tpArr.get(2).getAsDouble(),
        };
        float yaw = obj.has("yaw") ? obj.get("yaw").getAsFloat() : 0f;
        float pitch = obj.has("pitch") ? obj.get("pitch").getAsFloat() : 0f;
        return new PreviewShot(name, tp, yaw, pitch);
    }

    private static Optional<String> optString(JsonObject obj, String key) {
        return Optional.ofNullable(obj.get(key))
                .filter(JsonElement::isJsonPrimitive)
                .map(JsonElement::getAsString);
    }

    private static Optional<Integer> optInt(JsonObject obj, String key) {
        return Optional.ofNullable(obj.get(key))
                .filter(JsonElement::isJsonPrimitive)
                .map(JsonElement::getAsInt);
    }

    private static Optional<Boolean> optBool(JsonObject obj, String key) {
        return Optional.ofNullable(obj.get(key))
                .filter(JsonElement::isJsonPrimitive)
                .map(JsonElement::getAsBoolean);
    }
}
