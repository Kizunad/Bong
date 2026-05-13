package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.google.gson.JsonObject;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
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
    void sendMovementActionUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendMovementAction(ClientRequestProtocol.MovementAction.DASH);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"movement_action\",\"v\":1,\"action\":\"dash\"}",
            sent.get(0).body()
        );
    }

    @Test
    void sendMovementActionIncludesYawWhenProvided() {
        install();
        ClientRequestSender.sendMovementAction(ClientRequestProtocol.MovementAction.DASH, -45.0);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"movement_action\",\"v\":1,\"action\":\"dash\",\"yaw_degrees\":-45.0}",
            sent.get(0).body()
        );
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
    void sendSpiritNichePlaceUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendSpiritNichePlace(11, 64, 10, 4242L);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"spirit_niche_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":10,\"item_instance_id\":4242}",
            sent.get(0).body()
        );
    }

    @Test
    void sendCoffinOpenUsesCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendCoffinOpen(new BlockPos(0, 69, 0));
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"coffin_open\",\"v\":1,\"x\":0,\"y\":69,\"z\":0}", sent.get(0).body());
    }

    @Test
    void sendCoffinLifecycleUsesCorrectChannelAndJson() {
        install();
        BlockPos pos = new BlockPos(4, 65, -9);
        ClientRequestSender.sendCoffinPlace(pos, 4242L);
        ClientRequestSender.sendCoffinEnter(pos);
        ClientRequestSender.sendCoffinLeave();
        assertEquals(3, sent.size());
        assertEquals(
            "{\"type\":\"coffin_place\",\"v\":1,\"x\":4,\"y\":65,\"z\":-9,\"item_instance_id\":4242}",
            sent.get(0).body()
        );
        assertEquals(
            "{\"type\":\"coffin_enter\",\"v\":1,\"x\":4,\"y\":65,\"z\":-9}",
            sent.get(1).body()
        );
        assertEquals("{\"type\":\"coffin_leave\",\"v\":1}", sent.get(2).body());
    }

    @Test
    void sendAlchemyFurnaceRequestsUseCorrectChannelAndBlockPosJson() {
        install();
        BlockPos pos = new BlockPos(-12, 64, 38);

        ClientRequestSender.sendAlchemyOpenFurnace(pos);
        ClientRequestSender.sendAlchemyFurnacePlace(pos, 4242L);
        ClientRequestSender.sendAlchemyFeedSlot(pos, 0, "ci_she_hao", 3);

        assertEquals(3, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"alchemy_open_furnace\",\"v\":1,\"furnace_pos\":[-12,64,38]}",
            sent.get(0).body()
        );
        assertEquals(
            "{\"type\":\"alchemy_furnace_place\",\"v\":1,\"x\":-12,\"y\":64,\"z\":38,\"item_instance_id\":4242}",
            sent.get(1).body()
        );
        assertEquals(
            "{\"type\":\"alchemy_feed_slot\",\"v\":1,\"furnace_pos\":[-12,64,38],\"slot_idx\":0,\"material\":\"ci_she_hao\",\"count\":3}",
            sent.get(2).body()
        );
    }

    @Test
    void sendSpiritNicheRevealRequestsUseCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendSpiritNicheGaze(11, 64, 10);
        ClientRequestSender.sendSpiritNicheMarkCoordinate(12, 65, 11);
        assertEquals(2, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"spirit_niche_gaze\",\"v\":1,\"x\":11,\"y\":64,\"z\":10}",
            sent.get(0).body()
        );
        assertEquals(new Identifier("bong", "client_request"), sent.get(1).channel());
        assertEquals(
            "{\"type\":\"spirit_niche_mark_coordinate\",\"v\":1,\"x\":12,\"y\":65,\"z\":11}",
            sent.get(1).body()
        );
    }

    @Test
    void sendZhenfaRequestsUseCorrectChannelAndJson() {
        install();
        BlockPos pos = new BlockPos(11, 64, -3);
        ClientRequestSender.sendZhenfaPlace(
            pos,
            ClientRequestProtocol.ZhenfaKind.TRAP,
            ClientRequestProtocol.ZhenfaCarrierKind.NIGHT_WITHERED_VINE,
            0.3,
            "proximity"
        );
        ClientRequestSender.sendZhenfaPlace(
            pos,
            ClientRequestProtocol.ZhenfaKind.SLOW_TRAP,
            ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
            0.0,
            null,
            9002L,
            ClientRequestProtocol.ZhenfaTargetFace.TOP
        );
        ClientRequestSender.sendZhenfaTrigger(null);
        ClientRequestSender.sendZhenfaDisarm(pos, ClientRequestProtocol.ZhenfaDisarmMode.FORCE_BREAK);

        assertEquals(4, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"trap\",\"carrier\":\"night_withered_vine\",\"qi_invest_ratio\":0.3,\"trigger\":\"proximity\"}",
            sent.get(0).body()
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"slow_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.0,\"item_instance_id\":9002,\"target_face\":\"top\"}",
            sent.get(1).body()
        );
        assertEquals("{\"type\":\"zhenfa_trigger\",\"v\":1}", sent.get(2).body());
        assertEquals(
            "{\"type\":\"zhenfa_disarm\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"mode\":\"force_break\"}",
            sent.get(3).body()
        );
    }

    @Test
    void sendTradeOfferRequestsUseCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendTradeOfferRequest("entity:42", 1001L);
        ClientRequestSender.sendTradeOfferResponse("trade:a:b:1001:20", true, 2002L);

        assertEquals(2, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"trade_offer_request\",\"v\":1,\"target\":\"entity:42\",\"offered_instance_id\":1001}",
            sent.get(0).body()
        );
        assertEquals(new Identifier("bong", "client_request"), sent.get(1).channel());
        assertEquals(
            "{\"type\":\"trade_offer_response\",\"v\":1,\"offer_id\":\"trade:a:b:1001:20\",\"accepted\":true,\"requested_instance_id\":2002}",
            sent.get(1).body()
        );
    }

    @Test
    void sendSearchRequestsUseCorrectChannelAndJson() {
        install();
        ClientRequestSender.sendStartSearch(42L);
        ClientRequestSender.sendCancelSearch();

        assertEquals(2, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"start_search\",\"v\":1,\"container_entity_id\":42}",
            sent.get(0).body()
        );
        assertEquals(new Identifier("bong", "client_request"), sent.get(1).channel());
        assertEquals("{\"type\":\"cancel_search\",\"v\":1}", sent.get(1).body());
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
        ClientRequestSender.sendSkillBarCast(2, "entity:42");
        ClientRequestSender.sendSkillBarBindSkill(1, "burst_meridian.beng_quan");
        ClientRequestSender.sendSkillBarBindClear(1);
        ClientRequestSender.sendAnqiContainerSwitch(ClientRequestProtocol.AnqiContainerKind.QUIVER);

        assertEquals(5, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"skill_bar_cast\",\"v\":1,\"slot\":0}", sent.get(0).body());
        assertEquals("{\"type\":\"skill_bar_cast\",\"v\":1,\"slot\":2,\"target\":\"entity:42\"}", sent.get(1).body());
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":1,\"binding\":{\"kind\":\"skill\",\"skill_id\":\"burst_meridian.beng_quan\"}}",
            sent.get(2).body()
        );
        assertEquals("{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":1,\"binding\":null}", sent.get(3).body());
        assertEquals("{\"type\":\"anqi_container_switch\",\"v\":1,\"to\":\"quiver\"}", sent.get(4).body());
    }

    @Test
    void sendSkillConfigIntentUsesCorrectChannelAndJson() {
        install();
        JsonObject config = new JsonObject();
        config.addProperty("meridian_id", "Pericardium");
        config.addProperty("backfire_kind", "array");

        ClientRequestSender.sendSkillConfigIntent("zhenmai.sever_chain", config);

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"skill_config_intent\",\"v\":1,\"skill_id\":\"zhenmai.sever_chain\",\"config\":{\"meridian_id\":\"Pericardium\",\"backfire_kind\":\"array\"}}",
            sent.get(0).body()
        );
    }
}
