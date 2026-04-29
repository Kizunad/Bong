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
    void sendApplyPillSelfUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendApplyPillSelf(1001L);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1001,\"target\":{\"kind\":\"self\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void sendLearnSkillScrollUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendLearnSkillScroll(3003L);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"learn_skill_scroll\",\"v\":1,\"instance_id\":3003}",
            sent.get(0).body()
        );
    }

    @Test
    void sendInventoryMoveUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendInventoryMove(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.HotbarLoc(3)
        );
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},\"to\":{\"kind\":\"hotbar\",\"index\":3}}",
            sent.get(0).body()
        );
    }

    @Test
    void sendPickupDroppedItemUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendPickupDroppedItem(3003L);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"pickup_dropped_item\",\"v\":1,\"instance_id\":3003}",
            sent.get(0).body()
        );
    }

    @Test
    void sendMineralProbeUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendMineralProbe(8, 32, 8);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"mineral_probe\",\"v\":1,\"x\":8,\"y\":32,\"z\":8}",
            sent.get(0).body()
        );
    }

    @Test
    void sendInventoryDiscardItemUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendInventoryDiscardItem(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0)
        );
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_discard_item\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0}}",
            sent.get(0).body()
        );
    }

    @Test
    void sendDropWeaponUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendDropWeapon(
            2002L,
            new ClientRequestProtocol.EquipLoc("main_hand")
        );
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"drop_weapon_intent\",\"v\":1,\"instance_id\":2002,\"from\":{\"kind\":\"equip\",\"slot\":\"main_hand\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void sendRepairWeaponUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendRepairWeapon(4242L, 1, 64, 2);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"repair_weapon_intent\",\"v\":1,\"instance_id\":4242,\"station_pos\":[1,64,2]}",
            sent.get(0).body()
        );
    }

    @Test
    void sendForgeStationPlaceUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendForgeStationPlace(-12, 64, 38, 4242L, 2);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"forge_station_place\",\"v\":1,\"x\":-12,\"y\":64,\"z\":38,\"item_instance_id\":4242,\"station_tier\":2}",
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

    @Test
    void sendHeartDemonDecisionUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendHeartDemonDecision(1);
        ClientRequestSender.sendHeartDemonDecision(null);

        assertEquals(2, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"heart_demon_decision\",\"v\":1,\"choice_idx\":1}",
            sent.get(0).body()
        );
        assertEquals(
            "{\"type\":\"heart_demon_decision\",\"v\":1,\"choice_idx\":null}",
            sent.get(1).body()
        );
    }

    @Test
    void sendSkillBarRequestsUseCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendSkillBarCast(0);
        ClientRequestSender.sendSkillBarBindSkill(1, "burst_meridian.beng_quan");
        ClientRequestSender.sendSkillBarBindClear(1);

        assertEquals(3, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"skill_bar_cast\",\"v\":1,\"slot\":0}", sent.get(0).body());
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":1,\"binding\":{\"kind\":\"skill\",\"skill_id\":\"burst_meridian.beng_quan\"}}",
            sent.get(1).body()
        );
        assertEquals("{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":1,\"binding\":null}", sent.get(2).body());
    }
}
