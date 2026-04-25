package com.bong.client.network;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.google.gson.Gson;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

public class CultivationDetailHandlerTest {

    private final CultivationDetailHandler handler = new CultivationDetailHandler();

    @BeforeEach
    void setUp() { MeridianStateStore.resetForTests(); }

    @AfterEach
    void tearDown() { MeridianStateStore.resetForTests(); }

    private static ServerDataEnvelope envelope(JsonObject payload) {
        payload.addProperty("type", "cultivation_detail");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.length());
        assertTrue(r.isSuccess(), "fixture envelope should parse: " + r.errorMessage());
        return r.envelope();
    }

    private static JsonObject fullPayload(List<Boolean> opened, List<Double> rate, List<Double> cap, List<Double> integ) {
        JsonObject obj = new JsonObject();
        obj.add("opened", new Gson().toJsonTree(opened));
        obj.add("flow_rate", new Gson().toJsonTree(rate));
        obj.add("flow_capacity", new Gson().toJsonTree(cap));
        obj.add("integrity", new Gson().toJsonTree(integ));
        return obj;
    }

    private static <T> List<T> twenty(T value) {
        List<T> list = new ArrayList<>(20);
        for (int i = 0; i < 20; i++) list.add(value);
        return list;
    }

    @Test
    void appliesFullSnapshotToStore() {
        var payload = fullPayload(twenty(true), twenty(1.5), twenty(10.0), twenty(1.0));
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertNotNull(body);
        assertEquals(20, body.allChannels().size());
        ChannelState lu = body.channel(MeridianChannel.LU);
        assertEquals(10.0, lu.capacity());
        assertEquals(1.5, lu.currentFlow());
        assertEquals(ChannelState.DamageLevel.INTACT, lu.damage());
        assertFalse(lu.blocked());
    }

    @Test
    void unopenedChannelMarkedBlocked() {
        var opened = twenty(false);
        var payload = fullPayload(opened, twenty(0.0), twenty(5.0), twenty(1.0));
        handler.handle(envelope(payload));
        assertTrue(MeridianStateStore.snapshot().channel(MeridianChannel.HT).blocked());
    }

    @Test
    void integrityMapsToDamageLevels() {
        assertEquals(ChannelState.DamageLevel.INTACT,
            CultivationDetailHandler.damageFromIntegrity(0.99));
        assertEquals(ChannelState.DamageLevel.MICRO_TEAR,
            CultivationDetailHandler.damageFromIntegrity(0.80));
        assertEquals(ChannelState.DamageLevel.TORN,
            CultivationDetailHandler.damageFromIntegrity(0.40));
        assertEquals(ChannelState.DamageLevel.SEVERED,
            CultivationDetailHandler.damageFromIntegrity(0.05));
    }

    @Test
    void rejectsMissingArray() {
        JsonObject bad = new JsonObject();
        bad.add("opened", new Gson().toJsonTree(twenty(true)));
        // no flow_rate / flow_capacity / integrity
        var result = handler.handle(envelope(bad));
        assertFalse(result.handled());
        assertNull(MeridianStateStore.snapshot(), "store must not be touched on validation failure");
    }

    @Test
    void rejectsWrongArrayLength() {
        var payload = fullPayload(List.of(true, true), twenty(0.0), twenty(0.0), twenty(1.0));
        var result = handler.handle(envelope(payload));
        assertFalse(result.handled());
        assertTrue(result.logMessage().contains("array length mismatch"));
    }

    @Test
    void appliesCracksCountAndContaminationTotal() {
        var cracks = new ArrayList<Integer>();
        for (int i = 0; i < 20; i++) cracks.add(i == 4 ? 3 : 0); // HT 有 3 条裂痕
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(0.6));
        payload.addProperty("realm", "Condense");
        payload.add("cracks_count", new Gson().toJsonTree(cracks));
        payload.addProperty("contamination_total", 12.5);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(3, body.cracksFor(MeridianChannel.HT));
        assertEquals(0, body.cracksFor(MeridianChannel.LU));
        assertEquals(12.5, body.contaminationTotal(), 1e-9);
    }

    @Test
    void appliesLifespanPreviewWhenProvided() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        JsonObject lifespan = new JsonObject();
        lifespan.addProperty("years_lived", 74.5);
        lifespan.addProperty("cap_by_realm", 80);
        lifespan.addProperty("remaining_years", 5.5);
        lifespan.addProperty("death_penalty_years", 4);
        lifespan.addProperty("tick_rate_multiplier", 2.0);
        lifespan.addProperty("is_wind_candle", true);
        payload.add("lifespan", lifespan);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertTrue(body.hasLifespanPreview());
        assertEquals(74.5, body.yearsLived(), 1e-9);
        assertEquals(80, body.lifespanCapByRealm());
        assertEquals(5.5, body.remainingYears(), 1e-9);
        assertEquals(4, body.deathPenaltyYears());
        assertEquals(2.0, body.lifespanTickRateMultiplier(), 1e-9);
        assertTrue(body.isWindCandle());
    }

    @Test
    void appliesRealmAndOpenProgressWhenProvided() {
        var opened = twenty(false);
        var openProg = new ArrayList<Double>();
        for (int i = 0; i < 20; i++) openProg.add(i / 20.0);
        var payload = fullPayload(opened, twenty(0.0), twenty(5.0), twenty(1.0));
        payload.addProperty("realm", "Induce");
        payload.add("open_progress", new Gson().toJsonTree(openProg));
        payload.add("cracks_count", new Gson().toJsonTree(twenty(0)));
        payload.addProperty("contamination_total", 0.0);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals("Induce", body.realm());
        // LU=idx0 -> 0.0; HT=idx4 -> 0.2; BL=idx6 -> 0.3
        assertEquals(0.0, body.channel(MeridianChannel.LU).healProgress(), 1e-9);
        assertEquals(0.3, body.channel(MeridianChannel.BL).healProgress(), 1e-9);
    }

    @Test
    void openProgressIgnoredForOpenedChannels() {
        var opened = twenty(true);
        var openProg = new ArrayList<Double>();
        for (int i = 0; i < 20; i++) openProg.add(0.5);
        var payload = fullPayload(opened, twenty(1.0), twenty(5.0), twenty(1.0));
        payload.add("open_progress", new Gson().toJsonTree(openProg));
        handler.handle(envelope(payload));
        // 已打通经脉 blocked=false，healProgress 固定为 0（UI 不把 progress 渲染成治愈进度）
        assertEquals(0.0, MeridianStateStore.snapshot().channel(MeridianChannel.LU).healProgress(), 1e-9);
    }

    @Test
    void channelOrderExactly20() {
        assertEquals(20, CultivationDetailHandler.CHANNEL_ORDER.length);
        // head/tail sanity
        assertEquals(MeridianChannel.LU, CultivationDetailHandler.CHANNEL_ORDER[0]);
        assertEquals(MeridianChannel.LR, CultivationDetailHandler.CHANNEL_ORDER[11]);
        assertEquals(MeridianChannel.REN, CultivationDetailHandler.CHANNEL_ORDER[12]);
        assertEquals(MeridianChannel.YANG_WEI, CultivationDetailHandler.CHANNEL_ORDER[19]);
    }
}
