package com.bong.client.inventory;

import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InspectScreenDuXuTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        BongToast.resetForTests();
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

    @Test
    void meridianTargetRejectsOpenedAndFirstExtraordinary() {
        assertEquals("经脉数据加载中", InspectScreen.meridianTargetBlockReason(null, MeridianChannel.LU));
        assertTrue(InspectScreen.meridianTargetBlockReason(body("Awaken", null), MeridianChannel.LU)
            .contains("已通"));
        assertEquals(
            "首脉需先走十二正经",
            InspectScreen.meridianTargetBlockReason(bodyWithOpened("Awaken"), MeridianChannel.REN)
        );
        assertNull(InspectScreen.meridianTargetBlockReason(bodyWithOpened("Awaken"), MeridianChannel.LU));
    }

    @Test
    void dispatchSetMeridianTargetSendsOnlyWhenAllowed() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BodyInspectComponent bodyInspect = new BodyInspectComponent();
        bodyInspect.setMeridianBody(bodyWithOpened("Awaken"));
        bodyInspect.setSelectedChannel(MeridianChannel.LU);
        screen.setBodyInspectForTests(bodyInspect);

        assertTrue(screen.dispatchSetMeridianTarget());

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"set_meridian_target\",\"v\":1,\"meridian\":\"Lung\"}", sent.get(0).body());

        bodyInspect.setSelectedChannel(MeridianChannel.REN);
        assertFalse(screen.dispatchSetMeridianTarget());
        assertEquals(1, sent.size());
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

    private static MeridianBody bodyWithOpened(String realm, MeridianChannel... opened) {
        MeridianBody.Builder builder = MeridianBody.builder().realm(realm);
        for (MeridianChannel channel : MeridianChannel.values()) {
            boolean isOpened = contains(opened, channel);
            builder.channel(new ChannelState(
                channel,
                10.0,
                isOpened ? 10.0 : 0.0,
                ChannelState.DamageLevel.INTACT,
                0.0,
                0.0,
                !isOpened
            ));
        }
        return builder.build();
    }

    private static boolean contains(MeridianChannel[] values, MeridianChannel needle) {
        for (MeridianChannel value : values) {
            if (value == needle) {
                return true;
            }
        }
        return false;
    }
}
