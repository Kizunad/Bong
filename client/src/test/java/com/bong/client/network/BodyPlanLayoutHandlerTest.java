package com.bong.client.network;

import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** plan-race-system-v1 P2b — {@code body_plan_layout} payload 解析 + 缓存写入。 */
public class BodyPlanLayoutHandlerTest {

    private final BodyPlanLayoutHandler handler = new BodyPlanLayoutHandler();

    @BeforeEach
    void setUp() { BodyPlanLayoutStore.resetForTests(); }

    @AfterEach
    void tearDown() { BodyPlanLayoutStore.resetForTests(); }

    private static ServerDataEnvelope envelope(JsonObject payload) {
        payload.addProperty("type", "body_plan_layout");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.length());
        assertTrue(r.isSuccess(), "fixture envelope should parse: " + r.errorMessage());
        return r.envelope();
    }

    private static JsonObject point(double x, double y) {
        JsonObject p = new JsonObject();
        p.addProperty("x", x);
        p.addProperty("y", y);
        return p;
    }

    private static JsonObject fullPayload() {
        JsonObject payload = new JsonObject();
        payload.addProperty("body_plan_id", "humanoid");

        JsonArray silhouette = new JsonArray();
        JsonObject head = new JsonObject();
        head.addProperty("part_id", "head");
        JsonArray poly = new JsonArray();
        poly.add(point(0.4, 0.02));
        poly.add(point(0.6, 0.02));
        poly.add(point(0.6, 0.12));
        poly.add(point(0.4, 0.12));
        head.add("polygon", poly);
        silhouette.add(head);
        payload.add("silhouette", silhouette);

        JsonArray anchors = new JsonArray();
        JsonObject headAnchor = new JsonObject();
        headAnchor.addProperty("part_id", "head");
        headAnchor.add("point", point(0.5, 0.04));
        anchors.add(headAnchor);
        payload.add("anchors", anchors);

        JsonArray meridianPaths = new JsonArray();
        JsonObject lung = new JsonObject();
        lung.addProperty("channel_id", "lung");
        JsonArray pts = new JsonArray();
        pts.add(point(0.4, 0.2));
        pts.add(point(0.3, 0.3));
        lung.add("points", pts);
        meridianPaths.add(lung);
        payload.add("meridian_paths", meridianPaths);

        JsonArray displayMap = new JsonArray();
        JsonObject mapping = new JsonObject();
        mapping.addProperty("server_part_id", "head");
        mapping.addProperty("display_segment_id", "head");
        displayMap.add(mapping);
        payload.add("part_display_map", displayMap);

        JsonArray hudAnchors = new JsonArray();
        JsonObject headHudAnchor = new JsonObject();
        headHudAnchor.addProperty("part_id", "head");
        headHudAnchor.add("point", point(0.5, 0.053333));
        hudAnchors.add(headHudAnchor);
        payload.add("hud_anchors", hudAnchors);

        return payload;
    }

    @Test
    void parsesFullPayloadIntoStore() {
        var result = handler.handle(envelope(fullPayload()));
        assertTrue(result.handled(), result.logMessage());

        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        BodyPlanLayout layout = BodyPlanLayoutStore.current();
        assertEquals("humanoid", layout.bodyPlanId());
        assertEquals(1, layout.silhouette().size());
        assertEquals(1, layout.anchors().size());
        assertEquals(1, layout.meridianPaths().size());
        assertEquals(1, layout.partDisplayMap().size());
        assertEquals(1, layout.hudAnchors().size());
        assertEquals(0.5, layout.anchorFor("head").point().x());
        assertEquals(0.5, layout.hudAnchorFor("head").point().x());
        assertEquals(0.053333, layout.hudAnchorFor("head").point().y());
        assertEquals("head", layout.displaySegmentFor("head"));
    }

    @Test
    void missingHudAnchorsFieldDefaultsToEmptyNotCrash() {
        JsonObject payload = fullPayload();
        payload.remove("hud_anchors");
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        BodyPlanLayout layout = BodyPlanLayoutStore.current();
        assertTrue(layout.hudAnchors().isEmpty(),
            "missing hud_anchors field (non-humanoid plan / older server) must default to empty, not crash");
        assertNull(layout.hudAnchorFor("head"),
            "no hud_anchors means MiniBodyHudPlanner falls back to anchors scaling for this layout");
    }

    @Test
    void malformedHudAnchorEntryIsSkippedNotCrashed() {
        JsonObject payload = fullPayload();
        JsonArray hudAnchors = payload.getAsJsonArray("hud_anchors");
        JsonObject badHudAnchor = new JsonObject();
        badHudAnchor.addProperty("part_id", "ghost_part");
        // missing "point" — should be skipped, not crash
        hudAnchors.add(badHudAnchor);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertEquals(1, BodyPlanLayoutStore.current().hudAnchors().size(),
            "malformed hud_anchor entry (missing point) must be dropped, not crash the parse");
    }

    @Test
    void missingBodyPlanIdIsNoOp() {
        JsonObject payload = fullPayload();
        payload.remove("body_plan_id");
        var result = handler.handle(envelope(payload));
        assertFalse(result.handled());
        assertNull(BodyPlanLayoutStore.byId("humanoid"));
    }

    @Test
    void cachesUnderOwnIdWithoutTouchingCurrentPointerDirectly() {
        // Handler 只按 layout 自带 body_plan_id 建缓存；"当前"指针完全独立，
        // 由 CultivationDetailHandler 另行推进（见 BodyPlanLayoutStore 类头文档）。
        handler.handle(envelope(fullPayload()));
        assertNull(BodyPlanLayoutStore.currentPlanId(), "handler 不应该自行推进 current 指针");
        assertEquals("humanoid", BodyPlanLayoutStore.byId("humanoid").bodyPlanId());
    }

    @Test
    void malformedNestedEntriesAreSkippedNotCrashed() {
        JsonObject payload = fullPayload();
        JsonArray anchors = payload.getAsJsonArray("anchors");
        JsonObject badAnchor = new JsonObject();
        badAnchor.addProperty("part_id", "ghost_part");
        // missing "point" — should be skipped, not crash
        anchors.add(badAnchor);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertEquals(1, BodyPlanLayoutStore.current().anchors().size(),
            "malformed anchor entry (missing point) must be dropped, not crash the parse");
    }
}
