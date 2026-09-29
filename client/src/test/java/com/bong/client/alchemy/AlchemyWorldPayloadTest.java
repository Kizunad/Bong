package com.bong.client.alchemy;

import com.google.gson.JsonParser;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataRouter;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

class AlchemyWorldPayloadTest {
    @Test void rustFixturePreservesFurnaceQiSourceAndActualResult() throws Exception {
        var fixtures = JsonParser.parseString(Files.readString(Path.of("../proto/fixtures/alchemy_world_v1.json"))).getAsJsonArray();
        for (int index = 0; index < fixtures.size(); index++) {
            var bytes = Files.readAllBytes(Path.of("../proto/fixtures/alchemy_world_" + index + "_v1.pb"));
            var bridge = ProtoServerDataBridge.bridge(bytes);
            assertTrue(bridge.isSuccess(), bridge.errorMessage());
            var route = ServerDataRouter.createDefault().route(bridge.legacyJson(),
                bridge.legacyJson().getBytes(StandardCharsets.UTF_8).length);
            assertTrue(route.isHandled(), route.logMessage());
            assertEquals(AlchemyWorldPayload.parse(fixtures.get(index).toString()),
                AlchemyWorldPayload.parse(bridge.legacyJson()),
                "Rust 生产 protobuf 必须保留动作、零值状态及注元来源");
        }
        var qi = AlchemyWorldPayload.parse(fixtures.get(0).toString());
        assertEquals(new BlockPos(2, 64, 3), qi.position());
        assertEquals(.6, qi.heat());
        assertEquals(65.1, qi.source().y);
        assertEquals(2, qi.materials().get("ci_she_hao"));
        var result = AlchemyWorldPayload.parse(fixtures.get(1).toString());
        assertEquals(0, result.heat());
        assertEquals("alchemy_residue_processing_dregs", result.item());
        assertEquals("炮制药渣", result.name());
        assertEquals(AlchemyResultEffect.EARLY_TAKE, AlchemyWorldEffects.resultEffect(result.result()));
        var unsupported = fixtures.get(0).getAsJsonObject().deepCopy();
        unsupported.addProperty("v", 2);
        assertThrows(IllegalArgumentException.class, () -> AlchemyWorldPayload.parse(unsupported.toString()));
    }
}
