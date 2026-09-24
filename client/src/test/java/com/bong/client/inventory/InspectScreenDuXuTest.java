package com.bong.client.inventory;

import com.bong.client.cultivation.CultivationClientIntentSink;
import com.bong.client.cultivation.CultivationIntent;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import static org.junit.jupiter.api.Assertions.*;

/** 窗口迁移仍使用同一命令协议与目标准入，不允许按钮绕过境界或首脉限制。 */
class InspectScreenDuXuTest {
    private final List<String> sent = new ArrayList<>();
    private final CultivationClientIntentSink sink = new CultivationClientIntentSink();
    private void install(MeridianBody body) {
        MeridianStateStore.replace(body);
        ClientRequestSender.setBackendForTests((channel, bytes) -> sent.add(new String(bytes, StandardCharsets.UTF_8)));
    }
    @AfterEach void cleanup() {
        ClientRequestSender.resetBackendForTests();
        MeridianStateStore.clearOnDisconnect();
    }
    @Test void duXuRequiresRealmAndEveryChannelAndExplainsRejection() {
        var intent = new CultivationIntent(CultivationIntent.Action.DU_XU, null);
        install(body("Solidify", null));
        assertEquals("渡虚劫需通灵境", sink.dispatch(intent).reason());
        install(body("Spirit", MeridianChannel.DU));
        assertEquals("渡虚劫需打通全部经脉", sink.dispatch(intent).reason());
        assertTrue(sent.isEmpty(), "禁用条件不能发出渡虚命令");
        install(body("Spirit", null));
        sink.dispatch(intent);
        assertEquals(List.of("{\"type\":\"start_du_xu\",\"v\":1}"), sent);
    }
    @Test void targetUsesRealChannelIdAndRejectsMissingOpenedOrFirstExtraordinary() {
        var builder=MeridianBody.builder().realm("Awaken");
        for(var ch: MeridianChannel.values()) builder.channel(new ChannelState(ch,10,0,ChannelState.DamageLevel.INTACT,0,0,true));
        install(builder.build());
        sink.dispatch(new CultivationIntent(CultivationIntent.Action.TARGET, MeridianChannel.REN));
        assertTrue(sent.isEmpty(), "首脉不能是奇经");
        sink.dispatch(new CultivationIntent(CultivationIntent.Action.TARGET, MeridianChannel.LU));
        assertEquals(List.of("{\"type\":\"set_meridian_target\",\"v\":1,\"meridian\":\"lung\"}"), sent);
        install(body("Induce", null));
        assertNotNull(sink.dispatch(new CultivationIntent(CultivationIntent.Action.TARGET, MeridianChannel.LU)).reason());
        install(MeridianBody.builder().build());
        assertNotNull(sink.dispatch(new CultivationIntent(CultivationIntent.Action.TARGET, MeridianChannel.LU)).reason());
        assertEquals(1, sent.size(), "已通或当前构型没有的通道不能发送设目标");
    }
    @Test void forgeRejectsClosedChannelAndKeepsAxisPayload() {
        var intent=new CultivationIntent(CultivationIntent.Action.FORGE_RATE, MeridianChannel.LU);
        install(body("Induce", MeridianChannel.LU));
        assertNotNull(sink.dispatch(intent).reason());
        assertTrue(sent.isEmpty());
        install(body("Induce", null));
        sink.dispatch(intent);
        var json=com.google.gson.JsonParser.parseString(sent.get(0)).getAsJsonObject();
        assertEquals("forge_request", json.get("type").getAsString());
        assertEquals("lung", json.get("meridian").getAsString());
        assertEquals(com.bong.client.network.ClientRequestProtocol.ForgeAxis.Rate.name(), json.get("axis").getAsString());
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
