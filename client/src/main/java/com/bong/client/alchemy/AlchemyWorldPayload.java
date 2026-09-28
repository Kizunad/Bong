package com.bong.client.alchemy;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Vec3d;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/** 服务端 server_data.alchemy_world v1。火候是状态，动作只在首次收到该包时播放。 */
public record AlchemyWorldPayload(
    BlockPos position, double heat, boolean incense, Map<String, Integer> materials,
    String action, String item, int count, String result, String name, Vec3d source
) {
    private static final Set<String> ACTIONS = Set.of(
        "state", "ignite", "fire_raise", "fire_lower", "inject_qi", "feed", "incense", "collect");

    public static AlchemyWorldPayload parse(String json) {
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        if (root.get("v").getAsInt() != 1) throw new IllegalArgumentException("Unsupported alchemy world version");
        String action = root.get("action").getAsString();
        if (!ACTIONS.contains(action)) throw new IllegalArgumentException("Unknown alchemy action");
        var pos = root.getAsJsonArray("furnace_pos");
        if (pos.size() != 3) throw new IllegalArgumentException("Invalid furnace position");
        BlockPos position = new BlockPos(pos.get(0).getAsInt(), pos.get(1).getAsInt(), pos.get(2).getAsInt());
        double heat = root.get("heat").getAsDouble();
        if (!Double.isFinite(heat) || heat < 0 || heat > 1) throw new IllegalArgumentException("Invalid furnace heat");
        Map<String, Integer> materials = new LinkedHashMap<>();
        for (var entry : root.getAsJsonObject("materials").entrySet()) {
            int count = entry.getValue().getAsInt();
            if (count <= 0) throw new IllegalArgumentException("Invalid material count");
            materials.put(entry.getKey(), count);
        }
        Vec3d source = null;
        if (action.equals("inject_qi")) {
            var point = root.getAsJsonArray("source");
            if (point.size() != 3) throw new IllegalArgumentException("Invalid qi source");
            source = new Vec3d(point.get(0).getAsDouble(), point.get(1).getAsDouble(), point.get(2).getAsDouble());
            if (!Double.isFinite(source.x) || !Double.isFinite(source.y) || !Double.isFinite(source.z)) {
                throw new IllegalArgumentException("Non-finite qi source");
            }
        }
        String item = string(root, "item");
        int count = root.has("count") ? root.get("count").getAsInt() : 1;
        String result = string(root, "result");
        if (action.equals("feed") && (item.isBlank() || count <= 0)) throw new IllegalArgumentException("Invalid feed");
        if (action.equals("collect") && !Set.of("perfect", "good", "flawed", "waste", "early_take", "explode").contains(result)) {
            throw new IllegalArgumentException("Invalid alchemy result");
        }
        return new AlchemyWorldPayload(position, heat, root.get("incense").getAsBoolean(),
            Map.copyOf(materials), action, item, count, result, string(root, "name"), source);
    }

    private static String string(JsonObject root, String key) {
        return root.has(key) ? root.get(key).getAsString() : "";
    }
}
