package com.bong.client.network.alchemy;

import bong.Envelope;
import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataEnvelope;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

class AlchemyOutcomeResolvedHandlerTest {
    @AfterEach void clear() {
        AlchemyAttemptHistoryStore.clearOnDisconnect();
        AlchemyOutcomeForecastStore.clearOnDisconnect();
        ContaminationWarningStore.clearOnDisconnect();
    }

    @Test void pillDetailsSurviveTheWireProjectionWithoutInventingExplosionDamage() {
        var entry = receive(Envelope.AlchemyOutcomeResolved.newBuilder()
            .setBucket(Envelope.AlchemyOutcomeBucket.ALCHEMY_OUTCOME_BUCKET_GOOD)
            .setRecipeId("hui_yuan_pill_v0").setPill("hui_yuan_pill")
            .setQuality(.7).setToxinAmount(.3).setQiGain(18).build());
        assertEquals(.7, entry.quality());
        assertEquals(.3, entry.toxinAmount());
        assertEquals(18, entry.qiGain());
        assertNull(entry.damage(), "未下发的炸炉伤害必须保持未知，不伪造零值结果");
        assertNull(entry.meridianCrack());
    }

    @Test void explosionDetailsSurviveTheWireProjectionWithoutInventingAPillQuality() {
        var entry = receive(Envelope.AlchemyOutcomeResolved.newBuilder()
            .setBucket(Envelope.AlchemyOutcomeBucket.ALCHEMY_OUTCOME_BUCKET_EXPLODE)
            .setDamage(6).setMeridianCrack(.2).build());
        assertEquals(6, entry.damage());
        assertEquals(.2, entry.meridianCrack());
        assertNull(entry.quality(), "炸炉没有丹药成色，缺失字段必须保留为空");
        assertNull(entry.toxinAmount());
    }

    @Test void missingSnapshotsDoNotCreateDemoProbabilitiesOrToxinWarnings() {
        AlchemyOutcomeForecastStore.replace(null);
        ContaminationWarningStore.replace(null);
        var forecast = AlchemyOutcomeForecastStore.snapshot();
        assertEquals(0, forecast.perfectPct() + forecast.goodPct() + forecast.flawedPct()
            + forecast.wastePct() + forecast.explodePct(), "未收到预测时不得显示演示概率");
        var toxin = ContaminationWarningStore.snapshot();
        assertEquals(0, toxin.mellowMax() + toxin.violentMax(), "未收到丹毒时不得伪造阈值和超标预警");
    }

    private static AlchemyAttemptHistoryStore.Entry receive(Envelope.AlchemyOutcomeResolved outcome) {
        var wire = Envelope.ServerDataEnvelope.newBuilder().setAlchemyOutcomeResolved(outcome).build();
        var bridge = ProtoServerDataBridge.bridge(wire.toByteArray());
        assertTrue(bridge.isSuccess(), bridge.errorMessage());
        String json = bridge.legacyJson();
        var parsed = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), parsed.errorMessage());
        assertTrue(new AlchemyOutcomeResolvedHandler().handle(parsed.envelope()).handled());
        var entries = AlchemyAttemptHistoryStore.snapshot();
        return entries.get(entries.size() - 1);
    }
}
