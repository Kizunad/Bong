package com.bong.client.network;

import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.MeridianPath;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.model.bodyplan.PartDisplayMapping;
import com.bong.client.inventory.model.bodyplan.Point2;
import com.bong.client.inventory.model.bodyplan.SilhouettePart;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-race-system-v1 P2b — 解析服务端 {@code body_plan_layout} CustomPayload，翻译为
 * {@link BodyPlanLayout} 并推入 {@link BodyPlanLayoutStore}（按 {@code body_plan_id} 建缓存，
 * 不影响"当前"指针——指针由 {@link CultivationDetailHandler} 随
 * {@code cultivation_detail.body_plan_id} 单独更新，两者到达顺序不定，参见
 * {@link BodyPlanLayoutStore} 类头文档）。
 *
 * <p>缺 {@code body_plan_id} 或结构不合法时安全 no-op（server 侧
 * {@code deny_unknown_fields} + 校验已保证生产环境不会发出畸形 payload，这里只是
 * 防御性兜底，不 crash 客户端）。
 */
public final class BodyPlanLayoutHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        String bodyPlanId = readString(payload, "body_plan_id");
        if (bodyPlanId == null || bodyPlanId.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring body_plan_layout payload: missing body_plan_id"
            );
        }

        List<SilhouettePart> silhouette = parseSilhouette(readArray(payload, "silhouette"));
        List<PartAnchor> anchors = parseAnchors(readArray(payload, "anchors"));
        List<MeridianPath> meridianPaths = parseMeridianPaths(readArray(payload, "meridian_paths"));
        List<PartDisplayMapping> partDisplayMap = parsePartDisplayMap(readArray(payload, "part_display_map"));
        // plan-race-system-v1 P2 major 修复 —— 可选的第二锚点组（mini HUD 专用画布比例），
        // 缺该字段（未来非人 plan 常态）时 parseAnchors(null) 已经返回空 List，安全 no-op。
        List<PartAnchor> hudAnchors = parseAnchors(readArray(payload, "hud_anchors"));

        BodyPlanLayout layout =
            new BodyPlanLayout(bodyPlanId, silhouette, anchors, meridianPaths, partDisplayMap, hudAnchors);
        BodyPlanLayoutStore.putLayout(layout);

        return ServerDataDispatch.handled(
            envelope.type(),
            "Cached body_plan_layout for plan '" + bodyPlanId + "' ("
                + silhouette.size() + " silhouette parts, " + anchors.size() + " anchors, "
                + meridianPaths.size() + " meridian paths, " + partDisplayMap.size() + " display mappings, "
                + hudAnchors.size() + " hud anchors)"
        );
    }

    private static List<SilhouettePart> parseSilhouette(JsonArray array) {
        List<SilhouettePart> out = new ArrayList<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            String partId = readString(obj, "part_id");
            if (partId == null) continue;
            out.add(new SilhouettePart(partId, parsePoints(readArray(obj, "polygon"))));
        }
        return out;
    }

    private static List<PartAnchor> parseAnchors(JsonArray array) {
        List<PartAnchor> out = new ArrayList<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            String partId = readString(obj, "part_id");
            Point2 point = parsePoint(readObject(obj, "point"));
            if (partId == null || point == null) continue;
            out.add(new PartAnchor(partId, point));
        }
        return out;
    }

    private static List<MeridianPath> parseMeridianPaths(JsonArray array) {
        List<MeridianPath> out = new ArrayList<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            String channelId = readString(obj, "channel_id");
            if (channelId == null) continue;
            out.add(new MeridianPath(channelId, parsePoints(readArray(obj, "points"))));
        }
        return out;
    }

    private static List<PartDisplayMapping> parsePartDisplayMap(JsonArray array) {
        List<PartDisplayMapping> out = new ArrayList<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            String serverPartId = readString(obj, "server_part_id");
            String displaySegmentId = readString(obj, "display_segment_id");
            if (serverPartId == null || displaySegmentId == null) continue;
            out.add(new PartDisplayMapping(serverPartId, displaySegmentId));
        }
        return out;
    }

    private static List<Point2> parsePoints(JsonArray array) {
        List<Point2> out = new ArrayList<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            Point2 point = parsePoint(element.getAsJsonObject());
            if (point != null) out.add(point);
        }
        return out;
    }

    private static Point2 parsePoint(JsonObject obj) {
        if (obj == null) return null;
        JsonElement xEl = obj.get("x");
        JsonElement yEl = obj.get("y");
        if (xEl == null || yEl == null || !xEl.isJsonPrimitive() || !yEl.isJsonPrimitive()) return null;
        if (!xEl.getAsJsonPrimitive().isNumber() || !yEl.getAsJsonPrimitive().isNumber()) return null;
        return new Point2(xEl.getAsDouble(), yEl.getAsDouble());
    }

    private static JsonArray readArray(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    private static JsonObject readObject(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonObject()) ? el.getAsJsonObject() : null;
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }
}
