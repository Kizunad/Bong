package com.bong.client.network;

import com.bong.client.inventory.model.MorphEntry;
import com.bong.client.inventory.state.MorphStateStore;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 PR-5b — {@link MorphStateHandler} 解码 + 存入 {@link MorphStateStore}。
 *
 * <p>覆盖：① full 整表替换（含多实体）② full 空表清空旧数据 ③ delta active=true 插入
 * ④ delta active=false 移除 ⑤ 未知 mode 不崩溃不写表 ⑥ 缺 entity_id 的条目跳过
 * ⑦ route() 端到端解码。
 */
class MorphStateHandlerTest {

    private final MorphStateHandler handler = new MorphStateHandler();

    @BeforeEach
    void setUp() { MorphStateStore.resetForTests(); }

    @AfterEach
    void tearDown() { MorphStateStore.resetForTests(); }

    private static ServerDataEnvelope envelope(JsonObject payload) {
        payload.addProperty("type", "morph_state");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.length());
        assertTrue(r.isSuccess(), "fixture envelope should parse: " + r.errorMessage());
        return r.envelope();
    }

    private static JsonObject entry(int entityId, int modelKind, String formRaceId, String formBodyPlanId, boolean active) {
        JsonObject e = new JsonObject();
        e.addProperty("entity_id", entityId);
        e.addProperty("model_kind", modelKind);
        e.addProperty("form_race_id", formRaceId);
        e.addProperty("form_body_plan_id", formBodyPlanId);
        e.addProperty("active", active);
        return e;
    }

    @Test
    void fullModeReplacesTableWithMultipleEntities() {
        JsonArray entries = new JsonArray();
        entries.add(entry(42, 0, "whale", "whale", true));
        entries.add(entry(77, 0, "whale", "whale", true));
        JsonObject payload = new JsonObject();
        payload.addProperty("mode", "full");
        payload.add("entries", entries);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        Optional<MorphEntry> m42 = MorphStateStore.morphOf(42);
        assertTrue(m42.isPresent());
        assertEquals("whale", m42.get().formRaceId());
        assertTrue(MorphStateStore.morphOf(77).isPresent());
        assertTrue(MorphStateStore.morphOf(99).isEmpty(), "表里没有的 entity_id 应查不到");
    }

    @Test
    void fullModeWithEmptyEntriesClearsPreviousState() {
        JsonArray first = new JsonArray();
        first.add(entry(1, 0, "whale", "whale", true));
        JsonObject firstPayload = new JsonObject();
        firstPayload.addProperty("mode", "full");
        firstPayload.add("entries", first);
        handler.handle(envelope(firstPayload));
        assertTrue(MorphStateStore.morphOf(1).isPresent());

        JsonObject emptyPayload = new JsonObject();
        emptyPayload.addProperty("mode", "full");
        emptyPayload.add("entries", new JsonArray());
        var result = handler.handle(envelope(emptyPayload));
        assertTrue(result.handled(), result.logMessage());
        assertTrue(MorphStateStore.morphOf(1).isEmpty(), "空 full payload 应清空旧表（全量替换语义）");
    }

    @Test
    void deltaModeActiveTrueInsertsEntry() {
        JsonArray entries = new JsonArray();
        entries.add(entry(5, 0, "whale", "whale", true));
        JsonObject payload = new JsonObject();
        payload.addProperty("mode", "delta");
        payload.add("entries", entries);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        assertTrue(MorphStateStore.morphOf(5).isPresent());
        assertEquals("whale", MorphStateStore.morphOf(5).get().formRaceId());
    }

    @Test
    void deltaModeActiveFalseRemovesEntry() {
        // 先 full 插入一条，再用 delta active=false 移除。
        JsonArray full = new JsonArray();
        full.add(entry(5, 0, "whale", "whale", true));
        JsonObject fullPayload = new JsonObject();
        fullPayload.addProperty("mode", "full");
        fullPayload.add("entries", full);
        handler.handle(envelope(fullPayload));
        assertTrue(MorphStateStore.morphOf(5).isPresent());

        JsonArray delta = new JsonArray();
        // active=false 时文档约定其余字段恒空/0，仍然解码不 crash。
        delta.add(entry(5, 0, "", "", false));
        JsonObject deltaPayload = new JsonObject();
        deltaPayload.addProperty("mode", "delta");
        deltaPayload.add("entries", delta);
        var result = handler.handle(envelope(deltaPayload));

        assertTrue(result.handled(), result.logMessage());
        assertTrue(MorphStateStore.morphOf(5).isEmpty(), "active=false 应从表中移除该 entity_id");
    }

    @Test
    void deltaModeMixedAddAndRemoveInSamePayload() {
        JsonArray full = new JsonArray();
        full.add(entry(1, 0, "whale", "whale", true));
        JsonObject fullPayload = new JsonObject();
        fullPayload.addProperty("mode", "full");
        fullPayload.add("entries", full);
        handler.handle(envelope(fullPayload));

        JsonArray delta = new JsonArray();
        delta.add(entry(1, 0, "", "", false)); // 移除 1
        delta.add(entry(2, 0, "whale", "whale", true)); // 插入 2
        JsonObject deltaPayload = new JsonObject();
        deltaPayload.addProperty("mode", "delta");
        deltaPayload.add("entries", delta);
        handler.handle(envelope(deltaPayload));

        assertTrue(MorphStateStore.morphOf(1).isEmpty());
        assertTrue(MorphStateStore.morphOf(2).isPresent());
    }

    @Test
    void unknownModeDoesNotCrashAndLeavesStoreUntouched() {
        JsonArray full = new JsonArray();
        full.add(entry(1, 0, "whale", "whale", true));
        JsonObject fullPayload = new JsonObject();
        fullPayload.addProperty("mode", "full");
        fullPayload.add("entries", full);
        handler.handle(envelope(fullPayload));

        JsonObject weird = new JsonObject();
        weird.addProperty("mode", "future_mode_v2");
        weird.add("entries", new JsonArray());
        var result = handler.handle(envelope(weird));

        assertTrue(result.handled(), "未知 mode 应仍返回 handled（不是 parse 失败），只是忽略内容");
        assertTrue(MorphStateStore.morphOf(1).isPresent(), "未知 mode 不应改动既有表内容");
    }

    @Test
    void entryMissingEntityIdIsSkipped() {
        JsonObject noId = new JsonObject();
        noId.addProperty("model_kind", 0);
        noId.addProperty("form_race_id", "whale");
        noId.addProperty("form_body_plan_id", "whale");
        noId.addProperty("active", true);
        JsonArray entries = new JsonArray();
        entries.add(noId);
        entries.add(entry(9, 0, "whale", "whale", true));
        JsonObject payload = new JsonObject();
        payload.addProperty("mode", "full");
        payload.add("entries", entries);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        assertTrue(MorphStateStore.morphOf(9).isPresent(), "缺 entity_id 的条目跳过，不影响其余合法条目");
    }

    @Test
    void routeEndToEndDecodesMorphStateFull() {
        String json = "{"
            + "\"v\":1,\"type\":\"morph_state\",\"mode\":\"full\","
            + "\"entries\":[{\"entity_id\":42,\"model_kind\":0,\"form_race_id\":\"whale\","
            + "\"form_body_plan_id\":\"whale\",\"active\":true}]"
            + "}";
        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "valid JSON must parse: " + result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());
        assertTrue(MorphStateStore.morphOf(42).isPresent());
        assertEquals("whale", MorphStateStore.morphOf(42).get().formRaceId());
    }
}
