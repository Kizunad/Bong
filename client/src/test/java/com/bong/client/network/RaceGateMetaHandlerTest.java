package com.bong.client.network;

import com.bong.client.inventory.model.RaceGate;
import com.bong.client.inventory.state.RaceGateMetaStore;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P3c — {@link RaceGateMetaHandler} 解码 + 存入 {@link RaceGateMetaStore}。
 *
 * <p>覆盖：① happy path（两表 + 三档 gate）② 空表 ③ 未知 kind → fail-closed 哨兵
 * （P3 opus verify LOW 项：不再跳过，而是存入恒拒绝的 {@link RaceGate#blocked()}）
 * ④ 缺 id / 畸形条目跳过 ⑤ route() 端到端解码。
 */
class RaceGateMetaHandlerTest {

    private final RaceGateMetaHandler handler = new RaceGateMetaHandler();

    @BeforeEach
    void setUp() { RaceGateMetaStore.resetForTests(); }

    @AfterEach
    void tearDown() { RaceGateMetaStore.resetForTests(); }

    private static ServerDataEnvelope envelope(JsonObject payload) {
        payload.addProperty("type", "race_gate_meta");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.length());
        assertTrue(r.isSuccess(), "fixture envelope should parse: " + r.errorMessage());
        return r.envelope();
    }

    private static JsonObject entry(String id, String kind, List<String> species) {
        JsonObject gate = new JsonObject();
        gate.addProperty("kind", kind);
        JsonArray sp = new JsonArray();
        for (String s : species) sp.add(s);
        gate.add("species", sp);
        JsonObject e = new JsonObject();
        e.addProperty("id", id);
        e.add("gate", gate);
        return e;
    }

    @Test
    void decodesBothTablesWithAllThreeGateKinds() {
        JsonArray items = new JsonArray();
        items.add(entry("armor_whale_plate", "species", List.of("whale")));
        items.add(entry("human_mask", "humanoid", List.of()));
        JsonArray techniques = new JsonArray();
        techniques.add(entry("sword.cleave", "humanoid", List.of()));
        techniques.add(entry("fin.slap", "species", List.of("whale", "shark")));

        JsonObject payload = new JsonObject();
        payload.add("item_wearer_race", items);
        payload.add("technique_required_race", techniques);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        RaceGate armor = RaceGateMetaStore.gateForItem("armor_whale_plate");
        assertEquals("species", armor.kind());
        assertEquals(List.of("whale"), armor.species());
        assertEquals("humanoid", RaceGateMetaStore.gateForItem("human_mask").kind());

        assertEquals("humanoid", RaceGateMetaStore.gateForTechnique("sword.cleave").kind());
        RaceGate fin = RaceGateMetaStore.gateForTechnique("fin.slap");
        assertEquals("species", fin.kind());
        assertEquals(List.of("whale", "shark"), fin.species());

        assertTrue(RaceGateMetaStore.hasReceived());
    }

    @Test
    void emptyTablesAreValidAndClearStore() {
        JsonObject payload = new JsonObject();
        payload.add("item_wearer_race", new JsonArray());
        payload.add("technique_required_race", new JsonArray());
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        assertNull(RaceGateMetaStore.gateForItem("anything"), "空表查任何 id 都应 miss = any");
        assertNull(RaceGateMetaStore.gateForTechnique("anything"));
        assertTrue(RaceGateMetaStore.hasReceived(), "收到空 payload 也算 received");
    }

    @Test
    void unknownGateKindEntryIsStoredAsBlockedSentinelNotSkipped() {
        // P3 opus verify LOW 项：未知 kind 若被跳过，RaceGateMetaStore.gateForItem 会
        // miss → RaceGateEval 把 miss 判读成 any（恒放行）→ 未来新 kind 的装备/功法在
        // 未升级 client 上会被误判为"无门槛"，fail-open。修复后必须存入 blocked() 哨兵，
        // 命中一个恒拒绝的 gate（isAny()==false 且 allows() 恒 false）。
        JsonArray items = new JsonArray();
        items.add(entry("future_item", "cyborg", List.of()));   // 未来新 kind
        items.add(entry("human_mask", "humanoid", List.of()));  // 合法条目
        JsonObject payload = new JsonObject();
        payload.add("item_wearer_race", items);
        payload.add("technique_required_race", new JsonArray());
        handler.handle(envelope(payload));

        RaceGate blocked = RaceGateMetaStore.gateForItem("future_item");
        assertTrue(blocked != null, "未知 kind 条目不应被跳过——必须在表内命中一个哨兵 gate，实际 miss（fail-open 回归）");
        assertFalse(blocked.isAny(), "未知 kind 哨兵不是 any（不能被误判恒放行）");
        assertFalse(blocked.allows("whatever_race", true),
            "未知 kind 哨兵 allows() 必须恒 false（人形本体也不放行）");
        assertFalse(blocked.allows("whatever_race", false),
            "未知 kind 哨兵 allows() 必须恒 false（非人形本体也不放行）");
        assertEquals("humanoid", RaceGateMetaStore.gateForItem("human_mask").kind(),
            "同 payload 内的合法条目不受未知条目影响");
    }

    @Test
    void entryMissingIdIsSkipped() {
        JsonObject noId = new JsonObject();
        JsonObject gate = new JsonObject();
        gate.addProperty("kind", "humanoid");
        gate.add("species", new JsonArray());
        noId.add("gate", gate); // 无 id
        JsonArray items = new JsonArray();
        items.add(noId);
        items.add(entry("valid", "humanoid", List.of()));

        JsonObject payload = new JsonObject();
        payload.add("item_wearer_race", items);
        payload.add("technique_required_race", new JsonArray());
        handler.handle(envelope(payload));

        assertEquals("humanoid", RaceGateMetaStore.gateForItem("valid").kind(),
            "缺 id 条目跳过，不影响合法条目");
    }

    @Test
    void malformedEntryElementsAreSkipped() {
        JsonArray items = new JsonArray();
        items.add("not-an-object"); // 畸形元素
        items.add(entry("valid", "species", List.of("whale")));
        JsonObject payload = new JsonObject();
        payload.add("item_wearer_race", items);
        payload.add("technique_required_race", new JsonArray());
        handler.handle(envelope(payload));

        assertEquals("species", RaceGateMetaStore.gateForItem("valid").kind());
    }

    @Test
    void missingArrayFieldsTreatedAsEmpty() {
        // 缺两数组（畸形 JSON）：按空表处理，不 crash。
        JsonObject payload = new JsonObject();
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        assertNull(RaceGateMetaStore.gateForItem("x"));
        assertNull(RaceGateMetaStore.gateForTechnique("x"));
    }

    @Test
    void routeEndToEndDecodesRaceGateMeta() {
        String json = "{"
            + "\"v\":1,\"type\":\"race_gate_meta\","
            + "\"item_wearer_race\":[{\"id\":\"armor_whale_plate\",\"gate\":{\"kind\":\"species\",\"species\":[\"whale\"]}}],"
            + "\"technique_required_race\":[{\"id\":\"sword.cleave\",\"gate\":{\"kind\":\"humanoid\",\"species\":[]}}]"
            + "}";
        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "valid JSON must parse: " + result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());
        assertEquals("species", RaceGateMetaStore.gateForItem("armor_whale_plate").kind());
        assertEquals("humanoid", RaceGateMetaStore.gateForTechnique("sword.cleave").kind());
    }
}
