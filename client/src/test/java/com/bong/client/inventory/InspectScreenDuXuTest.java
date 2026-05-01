package com.bong.client.inventory;

import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InspectScreenDuXuTest {
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
    void duXuEligibilityRequiresSpiritAndAllMeridiansOpen() {
        assertTrue(InspectScreen.isDuXuEligible(body("Spirit", null)));
        assertFalse(InspectScreen.isDuXuEligible(body("Solidify", null)));
        assertFalse(InspectScreen.isDuXuEligible(body("Spirit", MeridianChannel.YIN_QIAO)));
        assertFalse(InspectScreen.isDuXuEligible(null));
    }

    @Test
    void dispatchStartDuXuSendsOnlyWhenEligible() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BodyInspectComponent bodyInspect = new BodyInspectComponent();
        bodyInspect.setMeridianBody(body("Spirit", null));
        screen.setBodyInspectForTests(bodyInspect);

        assertTrue(screen.dispatchStartDuXuIfEligible());

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"start_du_xu\",\"v\":1}", sent.get(0).body());
    }

    @Test
    void dispatchStartDuXuSkipsWhenNotEligible() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BodyInspectComponent bodyInspect = new BodyInspectComponent();
        bodyInspect.setMeridianBody(body("Spirit", MeridianChannel.DU));
        screen.setBodyInspectForTests(bodyInspect);

        assertFalse(screen.dispatchStartDuXuIfEligible());

        assertTrue(sent.isEmpty());
    }

    private static MeridianBody body(String realm, MeridianChannel blocked) {
        MeridianBody.Builder builder = MeridianBody.builder().realm(realm);
        for (MeridianChannel channel : MeridianChannel.values()) {
            builder.channel(new ChannelState(
                channel,
                10.0,
                blocked == channel ? 0.0 : 10.0,
                ChannelState.DamageLevel.INTACT,
                0.0,
                0.0,
                blocked == channel
            ));
        }
        return builder.build();
    }
}
