package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.google.gson.JsonObject;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class ClientRequestProtocolTest {

    @Test
    void encodesSetMeridianTarget() {
        String json = ClientRequestProtocol.encodeSetMeridianTarget(
            ClientRequestProtocol.MeridianId.Lung
        );
        assertEquals(
            "{\"type\":\"set_meridian_target\",\"v\":1,\"meridian\":\"Lung\"}",
            json
        );
    }

    @Test
    void encodesBreakthroughRequest() {
        String json = ClientRequestProtocol.encodeBreakthroughRequest();
        assertEquals("{\"type\":\"breakthrough_request\",\"v\":1}", json);
    }

    @Test
    void encodesAbortTribulationRequest() {
        String json = ClientRequestProtocol.encodeAbortTribulationRequest();
        assertEquals("{\"type\":\"abort_tribulation\",\"v\":1}", json);
    }

    @Test
    void encodesForgeRequestWithRateAxis() {
        String json = ClientRequestProtocol.encodeForgeRequest(
            ClientRequestProtocol.MeridianId.Ren,
            ClientRequestProtocol.ForgeAxis.Rate
        );
        assertEquals(
            "{\"type\":\"forge_request\",\"v\":1,\"meridian\":\"Ren\",\"axis\":\"Rate\"}",
            json
        );
    }

    @Test
    void encodesForgeRequestWithCapacityAxis() {
        String json = ClientRequestProtocol.encodeForgeRequest(
            ClientRequestProtocol.MeridianId.Du,
            ClientRequestProtocol.ForgeAxis.Capacity
        );
        assertEquals(
            "{\"type\":\"forge_request\",\"v\":1,\"meridian\":\"Du\",\"axis\":\"Capacity\"}",
            json
        );
    }

    @Test
    void encodesForgeFalseSkin() {
        String json = ClientRequestProtocol.encodeForgeFalseSkin(
            ClientRequestProtocol.FalseSkinKind.ROTTEN_WOOD_ARMOR
        );
        assertEquals(
            "{\"type\":\"forge_false_skin\",\"v\":1,\"kind\":\"rotten_wood_armor\"}",
            json
        );
    }

    @Test
    void encodesApplyPillSelf() {
        String json = ClientRequestProtocol.encodeApplyPillSelf(1001L);
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1001,\"target\":{\"kind\":\"self\"}}",
            json
        );
    }

    @Test
    void encodesApplyPillMeridianTarget() {
        String json = ClientRequestProtocol.encodeApplyPill(
            2002L,
            new ClientRequestProtocol.MeridianTarget(ClientRequestProtocol.MeridianId.Ren)
        );
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":2002,\"target\":{\"kind\":\"meridian\",\"meridian_id\":\"Ren\"}}",
            json
        );
    }

    @Test
    void encodesLearnSkillScroll() {
        String json = ClientRequestProtocol.encodeLearnSkillScroll(3003L);
        assertEquals(
            "{\"type\":\"learn_skill_scroll\",\"v\":1,\"instance_id\":3003}",
            json
        );
    }

    @Test
    void encodesSkillConfigIntent() {
        JsonObject config = new JsonObject();
        config.addProperty("meridian_id", "Pericardium");
        config.addProperty("backfire_kind", "tainted_yuan");

        assertEquals(
            "{\"type\":\"skill_config_intent\",\"v\":1,\"skill_id\":\"zhenmai.sever_chain\",\"config\":{\"meridian_id\":\"Pericardium\",\"backfire_kind\":\"tainted_yuan\"}}",
            ClientRequestProtocol.encodeSkillConfigIntent("zhenmai.sever_chain", config)
        );
    }

    @Test
    void encodesInventoryMoveFromContainerToHotbar() {
        String json = ClientRequestProtocol.encodeInventoryMove(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.HotbarLoc(3)
        );
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},\"to\":{\"kind\":\"hotbar\",\"index\":3}}",
            json
        );
    }

    @Test
    void encodesInventoryMoveFromEquipToContainer() {
        String json = ClientRequestProtocol.encodeInventoryMove(
            2002L,
            new ClientRequestProtocol.EquipLoc("main_hand"),
            new ClientRequestProtocol.ContainerLoc("small_pouch", 1, 2)
        );
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":2002,\"from\":{\"kind\":\"equip\",\"slot\":\"main_hand\"},\"to\":{\"kind\":\"container\",\"container_id\":\"small_pouch\",\"row\":1,\"col\":2}}",
            json
        );
    }

    @Test
    void encodesPickupDroppedItem() {
        String json = ClientRequestProtocol.encodePickupDroppedItem(3003L);
        assertEquals(
            "{\"type\":\"pickup_dropped_item\",\"v\":1,\"instance_id\":3003}",
            json
        );
    }

    @Test
    void encodesMineralProbe() {
        String json = ClientRequestProtocol.encodeMineralProbe(8, 32, 8);
        assertEquals(
            "{\"type\":\"mineral_probe\",\"v\":1,\"x\":8,\"y\":32,\"z\":8}",
            json
        );
    }

    @Test
    void encodesInventoryDiscardItem() {
        String json = ClientRequestProtocol.encodeInventoryDiscardItem(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0)
        );
        assertEquals(
            "{\"type\":\"inventory_discard_item\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0}}",
            json
        );
    }

    @Test
    void encodesDropWeapon() {
        String json = ClientRequestProtocol.encodeDropWeapon(
            2002L,
            new ClientRequestProtocol.EquipLoc("main_hand")
        );
        assertEquals(
            "{\"type\":\"drop_weapon_intent\",\"v\":1,\"instance_id\":2002,\"from\":{\"kind\":\"equip\",\"slot\":\"main_hand\"}}",
            json
        );
    }

    @Test
    void encodesRepairWeapon() {
        String json = ClientRequestProtocol.encodeRepairWeapon(4242L, 1, 64, 2);
        assertEquals(
            "{\"type\":\"repair_weapon_intent\",\"v\":1,\"instance_id\":4242,\"station_pos\":[1,64,2]}",
            json
        );
    }

    @Test
    void encodesForgeStationPlace() {
        String json = ClientRequestProtocol.encodeForgeStationPlace(-12, 64, 38, 4242L, 2);
        assertEquals(
            "{\"type\":\"forge_station_place\",\"v\":1,\"x\":-12,\"y\":64,\"z\":38,\"item_instance_id\":4242,\"station_tier\":2}",
            json
        );
    }

    @Test
    void encodesSpiritNichePlace() {
        String json = ClientRequestProtocol.encodeSpiritNichePlace(11, 64, 10, 4242L);
        assertEquals(
            "{\"type\":\"spirit_niche_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":10,\"item_instance_id\":4242}",
            json
        );
    }

    @Test
    void encodesCoffinOpen() {
        String json = ClientRequestProtocol.encodeCoffinOpen(new BlockPos(0, 69, 0));
        assertEquals("{\"type\":\"coffin_open\",\"v\":1,\"x\":0,\"y\":69,\"z\":0}", json);
    }

    @Test
    void encodesSpiritNicheRevealRequests() {
        assertEquals(
            "{\"type\":\"spirit_niche_gaze\",\"v\":1,\"x\":11,\"y\":64,\"z\":10}",
            ClientRequestProtocol.encodeSpiritNicheGaze(11, 64, 10)
        );
        assertEquals(
            "{\"type\":\"spirit_niche_mark_coordinate\",\"v\":1,\"x\":12,\"y\":65,\"z\":11}",
            ClientRequestProtocol.encodeSpiritNicheMarkCoordinate(12, 65, 11)
        );
    }

    @Test
    void encodesSpiritNicheActivateGuardianRequest() {
        assertEquals(
            "{\"type\":\"spirit_niche_activate_guardian\",\"v\":1,\"niche_pos\":[11,64,10],\"guardian_kind\":\"puppet\",\"materials\":[\"yi_shou_gu\",\"zhen_shi_zhong\"]}",
            ClientRequestProtocol.encodeSpiritNicheActivateGuardian(
                11,
                64,
                10,
                "puppet",
                java.util.List.of("yi_shou_gu", "zhen_shi_zhong")
            )
        );
    }

    @Test
    void encodesZhenfaRequests() {
        BlockPos pos = new BlockPos(11, 64, -3);
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"trap\",\"carrier\":\"night_withered_vine\",\"qi_invest_ratio\":0.3,\"trigger\":\"proximity\"}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.TRAP,
                ClientRequestProtocol.ZhenfaCarrierKind.NIGHT_WITHERED_VINE,
                0.3,
                "proximity"
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_trigger\",\"v\":1}",
            ClientRequestProtocol.encodeZhenfaTrigger(null)
        );
        assertEquals(
            "{\"type\":\"zhenfa_trigger\",\"v\":1,\"instance_id\":42}",
            ClientRequestProtocol.encodeZhenfaTrigger(42L)
        );
        assertEquals(
            "{\"type\":\"zhenfa_disarm\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"mode\":\"force_break\"}",
            ClientRequestProtocol.encodeZhenfaDisarm(pos, ClientRequestProtocol.ZhenfaDisarmMode.FORCE_BREAK)
        );
    }

    @Test
    void encodesSparringInviteResponse() {
        String json = ClientRequestProtocol.encodeSparringInviteResponse("sparring:1:a:b", true, false);
        assertEquals(
            "{\"type\":\"sparring_invite_response\",\"v\":1,\"invite_id\":\"sparring:1:a:b\",\"accepted\":true,\"timed_out\":false}",
            json
        );
    }

    @Test
    void encodesTradeOfferRequests() {
        assertEquals(
            "{\"type\":\"trade_offer_request\",\"v\":1,\"target\":\"entity:42\",\"offered_instance_id\":1001}",
            ClientRequestProtocol.encodeTradeOfferRequest("entity:42", 1001L)
        );
        assertEquals(
            "{\"type\":\"trade_offer_response\",\"v\":1,\"offer_id\":\"trade:a:b:1001:20\",\"accepted\":true,\"requested_instance_id\":2002}",
            ClientRequestProtocol.encodeTradeOfferResponse("trade:a:b:1001:20", true, 2002L)
        );
        assertEquals(
            "{\"type\":\"trade_offer_response\",\"v\":1,\"offer_id\":\"trade:a:b:1001:20\",\"accepted\":false}",
            ClientRequestProtocol.encodeTradeOfferResponse("trade:a:b:1001:20", false, null)
        );
    }

    @Test
    void encodesSearchRequests() {
        assertEquals(
            "{\"type\":\"start_search\",\"v\":1,\"container_entity_id\":42}",
            ClientRequestProtocol.encodeStartSearch(42L)
        );
        assertEquals(
            "{\"type\":\"cancel_search\",\"v\":1}",
            ClientRequestProtocol.encodeCancelSearch()
        );
    }

    @Test
    void encodesForgeTemperingHit() {
        String json = ClientRequestProtocol.encodeForgeTemperingHit(7L, ClientRequestProtocol.TemperBeat.L, 4);
        assertEquals(
            "{\"type\":\"forge_tempering_hit\",\"v\":1,\"session_id\":7,\"beat\":\"L\",\"ticks_remaining\":4}",
            json
        );
    }

    @Test
    void encodesForgeInscriptionScroll() {
        String json = ClientRequestProtocol.encodeForgeInscriptionScroll(7L, "sharp_v0");
        assertEquals(
            "{\"type\":\"forge_inscription_scroll\",\"v\":1,\"session_id\":7,\"inscription_id\":\"sharp_v0\"}",
            json
        );
    }

    @Test
    void encodesForgeConsecrationInject() {
        String json = ClientRequestProtocol.encodeForgeConsecrationInject(7L, 2.5);
        assertEquals(
            "{\"type\":\"forge_consecration_inject\",\"v\":1,\"session_id\":7,\"qi_amount\":2.5}",
            json
        );
    }

    @Test
    void encodesBotanyHarvestRequest() {
        String json = ClientRequestProtocol.encodeBotanyHarvestRequest("session-botany-01", BotanyHarvestMode.AUTO);
        assertEquals(
            "{\"type\":\"botany_harvest_request\",\"v\":1,\"session_id\":\"session-botany-01\",\"mode\":\"auto\"}",
            json
        );
    }

    @Test
    void encodesAlchemyFurnaceRequestsWithBlockPos() {
        BlockPos pos = new BlockPos(-12, 64, 38);

        assertEquals(
            "{\"type\":\"alchemy_open_furnace\",\"v\":1,\"furnace_pos\":[-12,64,38]}",
            ClientRequestProtocol.encodeAlchemyOpenFurnace(pos)
        );
        assertEquals(
            "{\"type\":\"alchemy_ignite\",\"v\":1,\"furnace_pos\":[-12,64,38],\"recipe_id\":\"kai_mai_pill_v0\"}",
            ClientRequestProtocol.encodeAlchemyIgnite(pos, "kai_mai_pill_v0")
        );
        assertEquals(
            "{\"type\":\"alchemy_feed_slot\",\"v\":1,\"furnace_pos\":[-12,64,38],\"slot_idx\":0,\"material\":\"ci_she_hao\",\"count\":3}",
            ClientRequestProtocol.encodeAlchemyFeedSlot(pos, 0, "ci_she_hao", 3)
        );
        assertEquals(
            "{\"type\":\"alchemy_take_back\",\"v\":1,\"furnace_pos\":[-12,64,38],\"slot_idx\":0}",
            ClientRequestProtocol.encodeAlchemyTakeBack(pos, 0)
        );
        assertEquals(
            "{\"type\":\"alchemy_intervention\",\"v\":1,\"furnace_pos\":[-12,64,38],\"intervention\":{\"kind\":\"inject_qi\",\"qi\":1.0}}",
            ClientRequestProtocol.encodeAlchemyInjectQi(pos, 1.0)
        );
        assertEquals(
            "{\"type\":\"alchemy_intervention\",\"v\":1,\"furnace_pos\":[-12,64,38],\"intervention\":{\"kind\":\"adjust_temp\",\"temp\":0.6}}",
            ClientRequestProtocol.encodeAlchemyAdjustTemp(pos, 0.6)
        );
        assertEquals(
            "{\"type\":\"alchemy_furnace_place\",\"v\":1,\"x\":-12,\"y\":64,\"z\":38,\"item_instance_id\":4242}",
            ClientRequestProtocol.encodeAlchemyFurnacePlace(pos, 4242L)
        );
    }

    @Test
    void encodesDuoSheRequest() {
        String json = ClientRequestProtocol.encodeDuoSheRequest("npc_12v0");
        assertEquals(
            "{\"type\":\"duo_she_request\",\"v\":1,\"target_id\":\"npc_12v0\"}",
            json
        );
    }

    @Test
    void encodesQiColorInspect() {
        String json = ClientRequestProtocol.encodeQiColorInspect("entity_bits:42");
        assertEquals(
            "{\"type\":\"qi_color_inspect\",\"v\":1,\"observed\":\"entity_bits:42\"}",
            json
        );
    }

    @Test
    void encodesUseLifeCore() {
        String json = ClientRequestProtocol.encodeUseLifeCore(4242L);
        assertEquals(
            "{\"type\":\"use_life_core\",\"v\":1,\"instance_id\":4242}",
            json
        );
    }

    @Test
    void encodesSelfAntidote() {
        assertEquals(
            "{\"type\":\"self_antidote\",\"v\":1,\"instance_id\":3003}",
            ClientRequestProtocol.encodeSelfAntidote(3003L)
        );
    }

    @Test
    void encodesSkillBarRequests() {
        assertEquals(
            "{\"type\":\"skill_bar_cast\",\"v\":1,\"slot\":0}",
            ClientRequestProtocol.encodeSkillBarCast(0)
        );
        assertEquals(
            "{\"type\":\"skill_bar_cast\",\"v\":1,\"slot\":0,\"target\":\"entity:42\"}",
            ClientRequestProtocol.encodeSkillBarCast(0, "entity:42")
        );
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":1,\"binding\":{\"kind\":\"skill\",\"skill_id\":\"burst_meridian.beng_quan\"}}",
            ClientRequestProtocol.encodeSkillBarBindSkill(1, "burst_meridian.beng_quan")
        );
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":2,\"binding\":{\"kind\":\"item\",\"template_id\":\"kai_mai_pill_v0\"}}",
            ClientRequestProtocol.encodeSkillBarBindItem(2, "kai_mai_pill_v0")
        );
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":3,\"binding\":null}",
            ClientRequestProtocol.encodeSkillBarBindClear(3)
        );
    }

    @Test
    void encodesExtractRequests() {
        assertEquals(
            "{\"type\":\"start_extract_request\",\"v\":1,\"portal_entity_id\":42}",
            ClientRequestProtocol.encodeStartExtractRequest(42L)
        );
        assertEquals(
            "{\"type\":\"cancel_extract_request\",\"v\":1}",
            ClientRequestProtocol.encodeCancelExtractRequest()
        );
    }

    @Test
    void meridianIdEnumCoversAll20Channels() {
        // 12 正经 + 8 奇经
        assertEquals(20, ClientRequestProtocol.MeridianId.values().length);
    }

    @Test
    void toMeridianIdMapsAllChannelsExhaustively() {
        // 所有 20 条 UI 通道均能映射为服务端 id，不抛 MatchException
        for (com.bong.client.inventory.model.MeridianChannel ch :
                com.bong.client.inventory.model.MeridianChannel.values()) {
            ClientRequestProtocol.MeridianId id = ClientRequestProtocol.toMeridianId(ch);
            assertEquals(true, id != null, "missing mapping for " + ch);
        }
    }

    @Test
    void encodesInsightDecisionChosen() {
        String json = ClientRequestProtocol.encodeInsightDecision("awaken_first", 2);
        assertEquals(
            "{\"type\":\"insight_decision\",\"v\":1,\"trigger_id\":\"awaken_first\",\"choice_idx\":2}",
            json
        );
    }

    @Test
    void encodesInsightDecisionDeclinedAsNull() {
        String json = ClientRequestProtocol.encodeInsightDecision("awaken_first", null);
        assertEquals(
            "{\"type\":\"insight_decision\",\"v\":1,\"trigger_id\":\"awaken_first\",\"choice_idx\":null}",
            json
        );
    }

    @Test
    void encodesHeartDemonDecisionChosen() {
        String json = ClientRequestProtocol.encodeHeartDemonDecision(2);
        assertEquals(
            "{\"type\":\"heart_demon_decision\",\"v\":1,\"choice_idx\":2}",
            json
        );
    }

    @Test
    void encodesHeartDemonDecisionTimeoutAsNull() {
        String json = ClientRequestProtocol.encodeHeartDemonDecision(null);
        assertEquals(
            "{\"type\":\"heart_demon_decision\",\"v\":1,\"choice_idx\":null}",
            json
        );
    }

    @Test
    void toMeridianIdMapsSampleChannels() {
        assertEquals(ClientRequestProtocol.MeridianId.Heart,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.HT));
        assertEquals(ClientRequestProtocol.MeridianId.Ren,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.REN));
        assertEquals(ClientRequestProtocol.MeridianId.YinWei,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.YIN_WEI));
        assertEquals(ClientRequestProtocol.MeridianId.TripleEnergizer,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.TE));
    }
}
