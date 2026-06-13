package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.google.gson.JsonObject;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

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
    void encodesMovementActionRequest() {
        String json = ClientRequestProtocol.encodeMovementAction(ClientRequestProtocol.MovementAction.DASH);
        assertEquals(
            "{\"type\":\"movement_action\",\"v\":1,\"action\":\"dash\"}",
            json,
            "expected movement_action without yaw_degrees because legacy dash requests omit yaw, actual: " + json
        );
    }

    @Test
    void encodesMovementActionRequestWithYaw() {
        String json = ClientRequestProtocol.encodeMovementAction(ClientRequestProtocol.MovementAction.DASH, 90.5);
        assertEquals(
            "{\"type\":\"movement_action\",\"v\":1,\"action\":\"dash\",\"yaw_degrees\":90.5}",
            json,
            "expected movement_action with finite yaw_degrees because dash direction needs the client yaw snapshot, actual: "
                + json
        );
    }

    @Test
    void rejectsNullMovementAction() {
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeMovementAction(null)
        );
    }

    @Test
    void rejectsNonFiniteMovementYaw() {
        for (double yawDegrees : List.of(Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY)) {
            assertThrows(
                IllegalArgumentException.class,
                () -> ClientRequestProtocol.encodeMovementAction(ClientRequestProtocol.MovementAction.DASH, yawDegrees),
                "expected IllegalArgumentException because yaw_degrees must be finite, actual no exception for "
                    + yawDegrees
            );
        }
    }

    @Test
    void encodesVoidActionSuppressTsy() {
        assertEquals(
            "{\"type\":\"void_action\",\"v\":1,\"request\":{\"kind\":\"suppress_tsy\",\"zone_id\":\"tsy_lingxu\"}}",
            ClientRequestProtocol.encodeVoidActionSuppressTsy("tsy_lingxu")
        );
    }

    @Test
    void encodesVoidActionBarrierCircle() {
        assertEquals(
            "{\"type\":\"void_action\",\"v\":1,\"request\":{\"kind\":\"barrier\",\"zone_id\":\"spawn\",\"geometry\":{\"kind\":\"circle\",\"center\":[1.0,64.0,2.0],\"radius\":24.0}}}",
            ClientRequestProtocol.encodeVoidActionBarrier("spawn", 1.0, 64.0, 2.0, 24.0)
        );
    }

    @Test
    void encodesVoidActionLegacyAssign() {
        assertEquals(
            "{\"type\":\"void_action\",\"v\":1,\"request\":{\"kind\":\"legacy_assign\",\"inheritor_id\":\"heir\",\"item_instance_ids\":[1001,1002],\"message\":\"留给后来人\"}}",
            ClientRequestProtocol.encodeVoidActionLegacyAssign("heir", List.of(1001L, 1002L), " 留给后来人 ")
        );
    }

    @Test
    void rejectsVoidActionBarrierWithBlankZone() {
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeVoidActionBarrier(" ", 1.0, 64.0, 2.0, 24.0)
        );
    }

    @Test
    void rejectsVoidActionBarrierWithNonPositiveRadius() {
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeVoidActionBarrier("spawn", 1.0, 64.0, 2.0, 0.0)
        );
    }

    @Test
    void rejectsVoidActionLegacyAssignWithNegativeItemId() {
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeVoidActionLegacyAssign("heir", List.of(-1L), "留给后来人")
        );
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
    void encodesBlockPlace() {
        String json = ClientRequestProtocol.encodeBlockPlace(
            new BlockPos(8, 64, 8),
            4242L,
            ClientRequestProtocol.ZhenfaTargetFace.NORTH
        );
        assertEquals(
            "{\"type\":\"block_place\",\"v\":1,\"x\":8,\"y\":64,\"z\":8,\"item_instance_id\":4242,\"target_face\":\"north\"}",
            json
        );
    }

    @Test
    void encodeBlockPlaceRejectsInvalidArguments() {
        BlockPos pos = new BlockPos(8, 64, 8);
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeBlockPlace(null, 4242L, ClientRequestProtocol.ZhenfaTargetFace.NORTH)
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeBlockPlace(pos, -1L, ClientRequestProtocol.ZhenfaTargetFace.NORTH)
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeBlockPlace(pos, 4242L, null)
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
    void encodesSpiritNicheRepair() {
        String json = ClientRequestProtocol.encodeSpiritNicheRepair(11, 64, 10, 4242L);
        assertEquals(
            "{\"type\":\"spirit_niche_repair\",\"v\":1,\"x\":11,\"y\":64,\"z\":10,\"item_instance_id\":4242}",
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
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"deceive_heaven\",\"carrier\":\"beast_core_inlaid\",\"qi_invest_ratio\":0.9}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.DECEIVE_HEAVEN,
                ClientRequestProtocol.ZhenfaCarrierKind.BEAST_CORE_INLAID,
                0.9,
                null
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"blast_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":1.0,\"item_instance_id\":9001,\"target_face\":\"north\"}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.BLAST_TRAP,
                ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
                1.0,
                null,
                9001L,
                ClientRequestProtocol.ZhenfaTargetFace.NORTH
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"beast_trap\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.0,\"item_instance_id\":9002,\"target_face\":\"north\"}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.BEAST_TRAP,
                ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
                0.0,
                null,
                9002L,
                ClientRequestProtocol.ZhenfaTargetFace.NORTH
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"trip_wire\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.0,\"item_instance_id\":9003,\"target_face\":\"north\"}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.TRIP_WIRE,
                ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
                0.0,
                null,
                9003L,
                ClientRequestProtocol.ZhenfaTargetFace.NORTH
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"kind\":\"decoy_stake\",\"carrier\":\"common_stone\",\"qi_invest_ratio\":0.0,\"item_instance_id\":9004,\"target_face\":\"top\"}",
            ClientRequestProtocol.encodeZhenfaPlace(
                pos,
                ClientRequestProtocol.ZhenfaKind.DECOY_STAKE,
                ClientRequestProtocol.ZhenfaCarrierKind.COMMON_STONE,
                0.0,
                null,
                9004L,
                ClientRequestProtocol.ZhenfaTargetFace.TOP
            )
        );
        assertEquals(
            "{\"type\":\"zhenfa_disarm\",\"v\":1,\"x\":11,\"y\":64,\"z\":-3,\"mode\":\"force_break\"}",
            ClientRequestProtocol.encodeZhenfaDisarm(pos, ClientRequestProtocol.ZhenfaDisarmMode.FORCE_BREAK)
        );
        assertEquals(
            "{\"type\":\"qi_scatter_bead_use\",\"v\":1,\"item_instance_id\":7001}",
            ClientRequestProtocol.encodeQiScatterBeadUse(7001L)
        );
        assertEquals(
            "{\"type\":\"qi_scatter_bead_use\",\"v\":1,\"item_instance_id\":7002,\"x\":11,\"y\":64,\"z\":-3}",
            ClientRequestProtocol.encodeQiScatterBeadUse(7002L, pos)
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeQiScatterBeadUse(-1L),
            "qi_scatter_bead_use 不能编码负 instance id"
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
    void encodesNpcEngagementRequests() {
        assertEquals(
            "{\"type\":\"npc_inspect_request\",\"v\":1,\"npc_entity_id\":42}",
            ClientRequestProtocol.encodeNpcInspectRequest(42)
        );
        assertEquals(
            "{\"type\":\"npc_dialogue_choice\",\"v\":1,\"npc_entity_id\":42,\"option_id\":\"trade\"}",
            ClientRequestProtocol.encodeNpcDialogueChoice(42, " trade ")
        );
        assertEquals(
            "{\"type\":\"npc_trade_request\",\"v\":1,\"npc_entity_id\":42,\"offered_items\":[1001,1002],\"requested_item_id\":\"spirit_grass\"}",
            ClientRequestProtocol.encodeNpcTradeRequest(42, List.of(1001L, 1002L), " spirit_grass ")
        );
    }

    @Test
    void rejectsInvalidNpcEngagementRequests() {
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeNpcInspectRequest(-1)
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeNpcDialogueChoice(1, " ")
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeNpcTradeRequest(1, List.of(-1L), "spirit_grass")
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeNpcTradeRequest(1, List.of(1L), " ")
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
    void encodesCraftStartQuantity() {
        assertEquals(
            "{\"type\":\"craft_start\",\"v\":1,\"recipe_id\":\"craft.example.herb_knife.iron\",\"quantity\":1}",
            ClientRequestProtocol.encodeCraftStart("craft.example.herb_knife.iron")
        );
        assertEquals(
            "{\"type\":\"craft_start\",\"v\":1,\"recipe_id\":\"craft.example.herb_knife.iron\",\"quantity\":3}",
            ClientRequestProtocol.encodeCraftStart("craft.example.herb_knife.iron", 3)
        );
        assertThrows(IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeCraftStart("craft.example.herb_knife.iron", 0));
        assertThrows(IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeCraftStart("craft.example.herb_knife.iron", 65));
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
    void encodesCoffinLifecycleRequests() {
        BlockPos pos = new BlockPos(4, 65, -9);

        assertEquals(
            "{\"type\":\"coffin_place\",\"v\":1,\"x\":4,\"y\":65,\"z\":-9,\"item_instance_id\":4242}",
            ClientRequestProtocol.encodeCoffinPlace(pos, 4242L)
        );
        assertEquals(
            "{\"type\":\"coffin_enter\",\"v\":1,\"x\":4,\"y\":65,\"z\":-9}",
            ClientRequestProtocol.encodeCoffinEnter(pos)
        );
        assertEquals(
            "{\"type\":\"coffin_leave\",\"v\":1}",
            ClientRequestProtocol.encodeCoffinLeave()
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
        assertEquals(
            "{\"type\":\"anqi_container_switch\",\"v\":1}",
            ClientRequestProtocol.encodeAnqiContainerSwitch()
        );
        assertEquals(
            "{\"type\":\"anqi_container_switch\",\"v\":1,\"to\":\"quiver\"}",
            ClientRequestProtocol.encodeAnqiContainerSwitch(ClientRequestProtocol.AnqiContainerKind.QUIVER)
        );
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeAnqiContainerSwitch(ClientRequestProtocol.AnqiContainerKind.FENGLINGHE)
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

    // ─── plan-supply-coffin-loot-ui P1：外部容器 C2S encode ──────────

    @Test
    void externalContainerMoveEncodesAllFields() {
        String json = ClientRequestProtocol.encodeExternalContainerMove(
            42, 100,
            new ClientRequestProtocol.ContainerLoc("ext_42", 0, 1),
            new ClientRequestProtocol.ContainerLoc("body_pocket", 2, 0)
        );
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals("external_container_move", obj.get("type").getAsString(),
            "type should be external_container_move");
        assertEquals(1, obj.get("v").getAsInt(), "version should be 1");
        assertEquals(42, obj.get("session_id").getAsLong(), "session_id should be 42");
        assertEquals(100, obj.get("instance_id").getAsLong(), "instance_id should be 100");

        com.google.gson.JsonObject from = obj.getAsJsonObject("from");
        assertEquals("container", from.get("kind").getAsString());
        assertEquals("ext_42", from.get("container_id").getAsString());
        assertEquals(0, from.get("row").getAsInt());
        assertEquals(1, from.get("col").getAsInt());

        com.google.gson.JsonObject to = obj.getAsJsonObject("to");
        assertEquals("container", to.get("kind").getAsString());
        assertEquals("body_pocket", to.get("container_id").getAsString());
        assertEquals(2, to.get("row").getAsInt(), "to.row should be 2");
        assertEquals(0, to.get("col").getAsInt(), "to.col should be 0");
    }

    @Test
    void externalContainerCloseEncodesSessionId() {
        String json = ClientRequestProtocol.encodeExternalContainerClose(99);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals("external_container_close", obj.get("type").getAsString(),
            "type should be external_container_close");
        assertEquals(1, obj.get("v").getAsInt(), "version should be 1");
        assertEquals(99, obj.get("session_id").getAsLong(), "session_id should be 99");
    }

    @Test
    void externalContainerMoveWithEquipLocation() {
        String json = ClientRequestProtocol.encodeExternalContainerMove(
            5, 200,
            new ClientRequestProtocol.ContainerLoc("ext_5", 2, 3),
            new ClientRequestProtocol.EquipLoc("main_hand")
        );
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        com.google.gson.JsonObject to = obj.getAsJsonObject("to");
        assertEquals("equip", to.get("kind").getAsString());
        assertEquals("main_hand", to.get("slot").getAsString());
    }

    // ─── plan-supply-coffin-loot-ui P2：supply_coffin_open ──────────

    @Test
    void encodesSupplyCoffinOpen() {
        String json = ClientRequestProtocol.encodeSupplyCoffinOpen(42);
        assertEquals(
            "{\"type\":\"supply_coffin_open\",\"v\":1,\"entity_id\":42}",
            json,
            "supply_coffin_open should encode type + v + entity_id"
        );
    }

    @Test
    void encodesSupplyCoffinOpenWithZeroId() {
        String json = ClientRequestProtocol.encodeSupplyCoffinOpen(0);
        assertEquals(
            "{\"type\":\"supply_coffin_open\",\"v\":1,\"entity_id\":0}",
            json,
            "entity_id 0 is a valid MC protocol entity id"
        );
    }

    @Test
    void encodesSupplyCoffinOpenWithNegativeId() {
        // Negative entity_id passes the protocol layer; server rejects at runtime.
        String json = ClientRequestProtocol.encodeSupplyCoffinOpen(-1);
        assertEquals(
            "{\"type\":\"supply_coffin_open\",\"v\":1,\"entity_id\":-1}",
            json,
            "negative entity_id should be encoded without client-side validation"
        );
    }

    @Test
    void encodesContainerOpen() {
        String json = ClientRequestProtocol.encodeContainerOpen(42);
        assertEquals(
            "{\"type\":\"container_open\",\"v\":1,\"entity_id\":42}",
            json,
            "container_open should encode type + v + entity_id"
        );
    }

    @Test
    void encodesContainerOpenWithZeroId() {
        String json = ClientRequestProtocol.encodeContainerOpen(0);
        assertEquals(
            "{\"type\":\"container_open\",\"v\":1,\"entity_id\":0}",
            json,
            "entity_id 0 is a valid MC protocol entity id"
        );
    }

    @Test
    void encodesContainerOpenWithNegativeId() {
        String json = ClientRequestProtocol.encodeContainerOpen(-1);
        assertEquals(
            "{\"type\":\"container_open\",\"v\":1,\"entity_id\":-1}",
            json,
            "negative entity_id should be encoded without client-side validation"
        );
    }

    // ─── plan-workbench-place-runtime-v1 P2：workbench_open ──────────

    @Test
    void encodesWorkbenchOpen() {
        String json = ClientRequestProtocol.encodeWorkbenchOpen(42);
        assertEquals(
            "{\"type\":\"workbench_open\",\"v\":1,\"entity_id\":42}",
            json,
            "workbench_open should encode type + v + entity_id"
        );
    }

    @Test
    void encodesWorkbenchOpenWithZeroId() {
        String json = ClientRequestProtocol.encodeWorkbenchOpen(0);
        assertEquals(
            "{\"type\":\"workbench_open\",\"v\":1,\"entity_id\":0}",
            json,
            "entity_id 0 is a valid MC protocol entity id"
        );
    }

    // ─── plan-coffin-tiers-v1 P3：C2S coffin_break / coffin_menu_reclaim ────

    @Test
    void encodesCoffinBreak() {
        String json = ClientRequestProtocol.encodeCoffinBreak(new BlockPos(10, 64, -5));
        assertEquals(
            "{\"type\":\"coffin_break\",\"v\":1,\"x\":10,\"y\":64,\"z\":-5}",
            json,
            "coffin_break should encode pos fields with correct type and negative z"
        );
    }

    @Test
    void encodesCoffinBreakOrigin() {
        String json = ClientRequestProtocol.encodeCoffinBreak(new BlockPos(0, 0, 0));
        assertEquals(
            "{\"type\":\"coffin_break\",\"v\":1,\"x\":0,\"y\":0,\"z\":0}",
            json,
            "coffin_break at origin should encode all-zero coordinates"
        );
    }

    @Test
    void encodesCoffinMenuReclaim() {
        String json = ClientRequestProtocol.encodeCoffinMenuReclaim(new BlockPos(3, 65, 7));
        assertEquals(
            "{\"type\":\"coffin_menu_reclaim\",\"v\":1,\"x\":3,\"y\":65,\"z\":7}",
            json,
            "coffin_menu_reclaim should encode pos fields with correct type"
        );
    }

    @Test
    void encodesCoffinMenuReclaimNegativeCoords() {
        String json = ClientRequestProtocol.encodeCoffinMenuReclaim(new BlockPos(-128, 0, -256));
        assertEquals(
            "{\"type\":\"coffin_menu_reclaim\",\"v\":1,\"x\":-128,\"y\":0,\"z\":-256}",
            json,
            "coffin_menu_reclaim should handle negative coordinates (valid world coords)"
        );
    }

    @Test
    void coffinBreakIncludesVersionField() {
        String json = ClientRequestProtocol.encodeCoffinBreak(new BlockPos(1, 64, 1));
        com.google.gson.JsonObject obj = new com.google.gson.JsonParser()
            .parse(json)
            .getAsJsonObject();
        assertEquals(1, obj.get("v").getAsInt(),
            "coffin_break must include version field v=1 for server decode");
        assertEquals("coffin_break", obj.get("type").getAsString(),
            "coffin_break type field must match server ClientRequestV1::CoffinBreak serde tag");
    }

    @Test
    void coffinMenuReclaimIncludesVersionField() {
        String json = ClientRequestProtocol.encodeCoffinMenuReclaim(new BlockPos(1, 64, 1));
        com.google.gson.JsonObject obj = new com.google.gson.JsonParser()
            .parse(json)
            .getAsJsonObject();
        assertEquals(1, obj.get("v").getAsInt(),
            "coffin_menu_reclaim must include version field v=1 for server decode");
        assertEquals("coffin_menu_reclaim", obj.get("type").getAsString(),
            "coffin_menu_reclaim type field must match server ClientRequestV1::CoffinMenuReclaim serde tag");
    }

    // ─── plan-exploration-probe-return-v1 P1：encodeFreshnessProbe encode ────

    @Test
    void encodesFreshnessProbe() {
        String json = ClientRequestProtocol.encodeFreshnessProbe(4242L);
        assertEquals(
            "{\"type\":\"freshness_probe\",\"v\":1,\"instance_id\":4242}",
            json,
            "freshness_probe 应包含 type='freshness_probe' + v=1 + instance_id，实际=" + json
        );
    }

    @Test
    void encodesFreshnessProbeTypeField() {
        String json = ClientRequestProtocol.encodeFreshnessProbe(1L);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals("freshness_probe", obj.get("type").getAsString(),
            "freshness_probe type 字段必须为 'freshness_probe'（与 server ClientRequestV1::FreshnessProbe serde tag 一致），实际=" + obj.get("type"));
    }

    @Test
    void encodesFreshnessProbeVersionField() {
        String json = ClientRequestProtocol.encodeFreshnessProbe(1L);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(1, obj.get("v").getAsInt(),
            "freshness_probe 必须包含 v=1（server serde 要求），实际=" + obj.get("v"));
    }

    @Test
    void encodesFreshnessProbeInstanceIdRoundTrip() {
        long instanceId = 9_999_999_999L;
        String json = ClientRequestProtocol.encodeFreshnessProbe(instanceId);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(instanceId, obj.get("instance_id").getAsLong(),
            "instance_id 应精确 round-trip，大值=" + instanceId + " 实际=" + obj.get("instance_id").getAsLong());
    }

    @Test
    void encodesFreshnessProbeZeroInstanceId() {
        String json = ClientRequestProtocol.encodeFreshnessProbe(0L);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(0L, obj.get("instance_id").getAsLong(),
            "instance_id=0 是合法值（server 端校验归属），客户端不拒绝，实际=" + obj.get("instance_id").getAsLong());
    }

    @Test
    void encodesFreshnessProbeNoExtraFields() {
        // 协议仅含 type / v / instance_id 三字段，不应有多余字段（防止 server serde 严格模式拒绝）
        String json = ClientRequestProtocol.encodeFreshnessProbe(7L);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(3, obj.size(),
            "freshness_probe 应恰好含 3 个字段（type/v/instance_id），实际 size=" + obj.size() + " json=" + json);
    }

    // ─── plan-exploration-probe-return-v1 P1：freshness_probe 边界值（S3 补充）───

    @Test
    void encodesFreshnessProbeMaxLong() {
        // Long.MAX_VALUE = 9223372036854775807（u64::MAX 近似上界，客户端透传不拒绝）
        long maxId = Long.MAX_VALUE;
        String json = ClientRequestProtocol.encodeFreshnessProbe(maxId);
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        // JSON long round-trip：Gson 读 getAsLong() 确保无精度丢失
        assertEquals(maxId, obj.get("instance_id").getAsLong(),
            "freshness_probe instance_id=Long.MAX_VALUE 应精确 round-trip，实际=" + obj.get("instance_id").getAsLong());
    }

    @Test
    void encodesFreshnessProbeNegativeInstanceId_throws() {
        // 负值 instance_id 不合法（与同文件其它 instance_id 编码器一致，抛 IllegalArgumentException）。
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeFreshnessProbe(-1L),
            "instance_id=-1 应抛 IllegalArgumentException（非负约束）"
        );
    }

    // ─── plan-shield-block-v1 P1 CR#3：encodeRaiseShield / encodeLowerShield encode tests ───

    @Test
    void encodesRaiseShieldTypeField() {
        String json = ClientRequestProtocol.encodeRaiseShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            "raise_shield",
            obj.get("type").getAsString(),
            "encodeRaiseShield type field must be 'raise_shield' to match server ClientRequestV1::RaiseShield serde tag, actual=" + obj.get("type")
        );
    }

    @Test
    void encodesRaiseShieldVersionField() {
        String json = ClientRequestProtocol.encodeRaiseShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            1,
            obj.get("v").getAsInt(),
            "encodeRaiseShield must include v=1 for server serde, actual=" + obj.get("v")
        );
    }

    @Test
    void encodesRaiseShieldExactPayload() {
        String json = ClientRequestProtocol.encodeRaiseShield();
        assertEquals(
            "{\"type\":\"raise_shield\",\"v\":1}",
            json,
            "encodeRaiseShield must produce minimal two-field payload matching server RaiseShield schema, actual=" + json
        );
    }

    @Test
    void encodesRaiseShieldNoExtraFields() {
        String json = ClientRequestProtocol.encodeRaiseShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            2,
            obj.size(),
            "encodeRaiseShield must contain exactly 2 fields (type + v); server serde uses additionalProperties:false, actual size=" + obj.size() + " json=" + json
        );
    }

    @Test
    void encodesLowerShieldTypeField() {
        String json = ClientRequestProtocol.encodeLowerShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            "lower_shield",
            obj.get("type").getAsString(),
            "encodeLowerShield type field must be 'lower_shield' to match server ClientRequestV1::LowerShield serde tag, actual=" + obj.get("type")
        );
    }

    @Test
    void encodesLowerShieldVersionField() {
        String json = ClientRequestProtocol.encodeLowerShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            1,
            obj.get("v").getAsInt(),
            "encodeLowerShield must include v=1 for server serde, actual=" + obj.get("v")
        );
    }

    @Test
    void encodesLowerShieldExactPayload() {
        String json = ClientRequestProtocol.encodeLowerShield();
        assertEquals(
            "{\"type\":\"lower_shield\",\"v\":1}",
            json,
            "encodeLowerShield must produce minimal two-field payload matching server LowerShield schema, actual=" + json
        );
    }

    @Test
    void encodesLowerShieldNoExtraFields() {
        String json = ClientRequestProtocol.encodeLowerShield();
        com.google.gson.JsonObject obj = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        assertEquals(
            2,
            obj.size(),
            "encodeLowerShield must contain exactly 2 fields (type + v); server serde uses additionalProperties:false, actual size=" + obj.size() + " json=" + json
        );
    }

    @Test
    void raiseShieldAndLowerShieldPayloadsAreDifferent() {
        // Smoke: raise and lower must produce different type literals — not the same payload.
        String raise = ClientRequestProtocol.encodeRaiseShield();
        String lower = ClientRequestProtocol.encodeLowerShield();
        assertNotNull(raise, "encodeRaiseShield must not return null");
        assertNotNull(lower, "encodeLowerShield must not return null");
        assertEquals(false, raise.equals(lower),
            "encodeRaiseShield and encodeLowerShield must produce different payloads — raise=" + raise + " lower=" + lower);
    }

    // ─── plan-worldgen-v4 P5 §8.1#5：encodeBlockPickerGive 双端契约 encode 测试 ───

    @Test
    void encodesBlockPickerGiveExactPayload() {
        String json = ClientRequestProtocol.encodeBlockPickerGive("stone_bricks", 16);
        assertEquals(
            "{\"type\":\"block_picker_give\",\"v\":1,\"block_id\":\"stone_bricks\",\"count\":16}",
            json,
            "encodeBlockPickerGive must match TS BlockPickerActionV1 / Rust BlockPickerGive wire shape, actual=" + json
        );
    }

    @Test
    void encodeBlockPickerGiveTypeAndVersionMatchServerSerde() {
        JsonObject obj = com.google.gson.JsonParser
            .parseString(ClientRequestProtocol.encodeBlockPickerGive("dirt", 1)).getAsJsonObject();
        assertEquals("block_picker_give", obj.get("type").getAsString(),
            "type field must be 'block_picker_give' to match server ClientRequestV1::BlockPickerGive serde tag, actual=" + obj.get("type"));
        assertEquals(1, obj.get("v").getAsInt(),
            "v must be 1 for server serde, actual=" + obj.get("v"));
        assertEquals(4, obj.size(),
            "payload must contain exactly type+v+block_id+count (schema additionalProperties:false), actual size=" + obj.size() + " json=" + obj);
    }

    @Test
    void encodeBlockPickerGiveAcceptsCountBoundaries() {
        assertEquals(
            "{\"type\":\"block_picker_give\",\"v\":1,\"block_id\":\"sand\",\"count\":1}",
            ClientRequestProtocol.encodeBlockPickerGive("sand", 1),
            "count=1 is the lower bound and must encode cleanly"
        );
        assertEquals(
            "{\"type\":\"block_picker_give\",\"v\":1,\"block_id\":\"sand\",\"count\":64}",
            ClientRequestProtocol.encodeBlockPickerGive("sand", ClientRequestProtocol.BLOCK_PICKER_MAX_COUNT),
            "count=64 is the upper bound (one stack) and must encode cleanly"
        );
    }

    @Test
    void encodeBlockPickerGiveRejectsBlankBlockId() {
        for (String bad : List.of("", "   ")) {
            assertThrows(
                IllegalArgumentException.class,
                () -> ClientRequestProtocol.encodeBlockPickerGive(bad, 1),
                "blank block_id must be rejected because schema requires minLength 1, input=[" + bad + "]"
            );
        }
        assertThrows(
            IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeBlockPickerGive(null, 1),
            "null block_id must be rejected before send"
        );
    }

    @Test
    void encodeBlockPickerGiveRejectsCountOutOfRange() {
        for (int bad : new int[] {0, -1, 65, 1000}) {
            assertThrows(
                IllegalArgumentException.class,
                () -> ClientRequestProtocol.encodeBlockPickerGive("stone", bad),
                "count outside [1,64] must be rejected because schema bounds count to a stack, count=" + bad
            );
        }
    }
}
