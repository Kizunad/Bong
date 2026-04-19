package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class ClientRequestSenderTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void sendSetMeridianTargetUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendSetMeridianTarget(ClientRequestProtocol.MeridianId.Heart);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"set_meridian_target\",\"v\":1,\"meridian\":\"Heart\"}",
            sent.get(0).body()
        );
    }

    @Test
    void sendBreakthroughRequestMinimalBody() {
        install();
        ClientRequestSender.sendBreakthroughRequest();
        assertEquals(1, sent.size());
        assertEquals("{\"type\":\"breakthrough_request\",\"v\":1}", sent.get(0).body());
    }

    @Test
    void sendForgeRequestIncludesMeridianAndAxis() {
        install();
        ClientRequestSender.sendForgeRequest(
            ClientRequestProtocol.MeridianId.Kidney,
            ClientRequestProtocol.ForgeAxis.Capacity
        );
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"forge_request\",\"v\":1,\"meridian\":\"Kidney\",\"axis\":\"Capacity\"}",
            sent.get(0).body()
        );
    }

    @Test
    void sendBotanyHarvestRequestIncludesSessionAndMode() {
        install();
        ClientRequestSender.sendBotanyHarvestRequest("session-botany-01", BotanyHarvestMode.MANUAL);
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"botany_harvest_request\",\"v\":1,\"session_id\":\"session-botany-01\",\"mode\":\"manual\"}",
            sent.get(0).body()
        );
    }
}
