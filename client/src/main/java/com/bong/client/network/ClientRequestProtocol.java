package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.cultivation.voidaction.VoidActionKind;
import com.bong.client.inventory.model.MeridianChannel;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.util.math.BlockPos;

import java.util.List;

/**
 * 客户端 → 服务端 {@code bong:client_request} 通道的协议常量与 JSON 编码。
 *
 * <p>与 Rust {@code server/src/schema/client_request.rs} 和 TypeScript
 * {@code agent/packages/schema/src/client-request.ts} 1:1 对齐。</p>
 *
 * <p>消息形状：{@code {"type": "<snake_case>", "v": 1, ...}}。</p>
 */
public final class ClientRequestProtocol {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "client_request";
    public static final int VERSION = 1;
    public static final int MAX_CRAFT_QUANTITY = 64;

    /** 服务端 {@code MeridianId} 的 PascalCase 字面量（serde 默认序列化）。 */
    public enum MeridianId {
        // 12 正经
        Lung, LargeIntestine, Stomach, Spleen, Heart, SmallIntestine,
        Bladder, Kidney, Pericardium, TripleEnergizer, Gallbladder, Liver,
        // 8 奇经
        Ren, Du, Chong, Dai, YinQiao, YangQiao, YinWei, YangWei
    }

    /** 服务端 {@code ForgeAxis}（serde 默认 PascalCase）。 */
    public enum ForgeAxis { Rate, Capacity }

    public enum FalseSkinKind {
        SPIDER_SILK("spider_silk"),
        ROTTEN_WOOD_ARMOR("rotten_wood_armor");

        private final String wireName;

        FalseSkinKind(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    /** 淬炼击键：J=Light, K=Heavy, L=Fold。 */
    public enum TemperBeat { L, H, F }

    public enum AnqiContainerKind {
        HAND_SLOT("hand_slot"),
        QUIVER("quiver"),
        POCKET_POUCH("pocket_pouch"),
        FENGLINGHE("fenglinghe");

        private final String wireName;

        AnqiContainerKind(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum MovementAction {
        DASH("dash");

        private final String wireName;

        MovementAction(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum ZhenfaKind {
        TRAP("trap"),
        WARD("ward"),
        WARNING_TRAP("warning_trap"),
        BLAST_TRAP("blast_trap"),
        SLOW_TRAP("slow_trap"),
        SHRINE_WARD("shrine_ward"),
        LINGJU("lingju"),
        DECEIVE_HEAVEN("deceive_heaven"),
        ILLUSION("illusion");

        private final String wireName;

        ZhenfaKind(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum ZhenfaTargetFace {
        TOP("top"),
        BOTTOM("bottom"),
        NORTH("north"),
        SOUTH("south"),
        EAST("east"),
        WEST("west");

        private final String wireName;

        ZhenfaTargetFace(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum ZhenfaCarrierKind {
        COMMON_STONE("common_stone"),
        LINGQI_BLOCK("lingqi_block"),
        NIGHT_WITHERED_VINE("night_withered_vine"),
        BEAST_CORE_INLAID("beast_core_inlaid");

        private final String wireName;

        ZhenfaCarrierKind(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum ZhenfaDisarmMode {
        DISARM("disarm"),
        FORCE_BREAK("force_break");

        private final String wireName;

        ZhenfaDisarmMode(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }
    }

    private ClientRequestProtocol() {}

    /** 将 UI 侧 {@link MeridianChannel} 枚举映射为服务端 {@link MeridianId}。 */
    public static MeridianId toMeridianId(MeridianChannel ch) {
        return switch (ch) {
            case LU -> MeridianId.Lung;
            case LI -> MeridianId.LargeIntestine;
            case ST -> MeridianId.Stomach;
            case SP -> MeridianId.Spleen;
            case HT -> MeridianId.Heart;
            case SI -> MeridianId.SmallIntestine;
            case BL -> MeridianId.Bladder;
            case KI -> MeridianId.Kidney;
            case PC -> MeridianId.Pericardium;
            case TE -> MeridianId.TripleEnergizer;
            case GB -> MeridianId.Gallbladder;
            case LR -> MeridianId.Liver;
            case REN -> MeridianId.Ren;
            case DU -> MeridianId.Du;
            case CHONG -> MeridianId.Chong;
            case DAI -> MeridianId.Dai;
            case YIN_QIAO -> MeridianId.YinQiao;
            case YANG_QIAO -> MeridianId.YangQiao;
            case YIN_WEI -> MeridianId.YinWei;
            case YANG_WEI -> MeridianId.YangWei;
        };
    }

    public static String encodeSetMeridianTarget(MeridianId meridian) {
        JsonObject obj = envelope("set_meridian_target");
        obj.addProperty("meridian", meridian.name());
        return obj.toString();
    }

    public static String encodeBreakthroughRequest() {
        return envelope("breakthrough_request").toString();
    }

    public static String encodeStartDuXuRequest() {
        return envelope("start_du_xu").toString();
    }

    public static String encodeAbortTribulationRequest() {
        return envelope("abort_tribulation").toString();
    }

    public static String encodeVoidActionSuppressTsy(String zoneId) {
        JsonObject request = voidActionRequest(VoidActionKind.SUPPRESS_TSY);
        request.addProperty("zone_id", requireNonBlank(zoneId, "zoneId"));
        return encodeVoidAction(request);
    }

    public static String encodeVoidActionExplodeZone(String zoneId) {
        JsonObject request = voidActionRequest(VoidActionKind.EXPLODE_ZONE);
        request.addProperty("zone_id", requireNonBlank(zoneId, "zoneId"));
        return encodeVoidAction(request);
    }

    public static String encodeVoidActionBarrier(String zoneId, double centerX, double centerY, double centerZ, double radius) {
        if (!Double.isFinite(centerX) || !Double.isFinite(centerY) || !Double.isFinite(centerZ)) {
            throw new IllegalArgumentException("barrier center must be finite");
        }
        if (!Double.isFinite(radius) || radius <= 0.0) {
            throw new IllegalArgumentException("barrier radius must be finite and > 0, got " + radius);
        }
        JsonObject request = voidActionRequest(VoidActionKind.BARRIER);
        request.addProperty("zone_id", requireNonBlank(zoneId, "zoneId"));
        JsonObject geometry = new JsonObject();
        geometry.addProperty("kind", "circle");
        JsonArray center = new JsonArray();
        center.add(centerX);
        center.add(centerY);
        center.add(centerZ);
        geometry.add("center", center);
        geometry.addProperty("radius", radius);
        request.add("geometry", geometry);
        return encodeVoidAction(request);
    }

    public static String encodeVoidActionLegacyAssign(String inheritorId, List<Long> itemInstanceIds, String message) {
        JsonObject request = voidActionRequest(VoidActionKind.LEGACY_ASSIGN);
        request.addProperty("inheritor_id", requireNonBlank(inheritorId, "inheritorId"));
        JsonArray items = new JsonArray();
        if (itemInstanceIds != null) {
            for (Long instanceId : itemInstanceIds) {
                if (instanceId == null || instanceId < 0) {
                    throw new IllegalArgumentException("itemInstanceIds must contain only non-negative ids");
                }
                items.add(instanceId.longValue());
            }
        }
        request.add("item_instance_ids", items);
        if (message == null || message.isBlank()) {
            request.add("message", com.google.gson.JsonNull.INSTANCE);
        } else {
            request.addProperty("message", message.trim());
        }
        return encodeVoidAction(request);
    }

    /** 心魔劫决定 C2S 回执。{@code chosenIdx = null} 表示超时或未选。 */
    public static String encodeHeartDemonDecision(Integer chosenIdx) {
        JsonObject obj = envelope("heart_demon_decision");
        if (chosenIdx == null) {
            obj.add("choice_idx", com.google.gson.JsonNull.INSTANCE);
        } else {
            if (chosenIdx < 0) {
                throw new IllegalArgumentException("chosenIdx must be >= 0, got " + chosenIdx);
            }
            obj.addProperty("choice_idx", chosenIdx.intValue());
        }
        return obj.toString();
    }

    /**
     * 顿悟决定 C2S 回执。{@code chosenIdx = null} 表示拒绝或超时；否则为选中候选下标（0-based）。
     */
    public static String encodeInsightDecision(String triggerId, Integer chosenIdx) {
        JsonObject obj = envelope("insight_decision");
        obj.addProperty("trigger_id", triggerId);
        if (chosenIdx == null) {
            obj.add("choice_idx", com.google.gson.JsonNull.INSTANCE);
        } else {
            if (chosenIdx < 0) {
                throw new IllegalArgumentException("chosenIdx must be >= 0, got " + chosenIdx);
            }
            obj.addProperty("choice_idx", chosenIdx.intValue());
        }
        return obj.toString();
    }

    public static String encodeForgeRequest(MeridianId meridian, ForgeAxis axis) {
        JsonObject obj = envelope("forge_request");
        obj.addProperty("meridian", meridian.name());
        obj.addProperty("axis", axis.name());
        return obj.toString();
    }

    public static String encodeBotanyHarvestRequest(String sessionId, BotanyHarvestMode mode) {
        if (sessionId == null || sessionId.isBlank()) {
            throw new IllegalArgumentException("sessionId must not be blank");
        }
        if (mode == null) {
            throw new IllegalArgumentException("mode must not be null");
        }
        JsonObject obj = envelope("botany_harvest_request");
        obj.addProperty("session_id", sessionId);
        obj.addProperty("mode", mode.wireName());
        return obj.toString();
    }

    public static String encodeCombatReincarnate() {
        return envelope("combat_reincarnate").toString();
    }

    public static String encodeCombatTerminate() {
        return envelope("combat_terminate").toString();
    }

    public static String encodeCombatCreateNewCharacter() {
        return envelope("combat_create_new_character").toString();
    }

    public static String encodeDuoSheRequest(String targetId) {
        if (targetId == null || targetId.isBlank()) {
            throw new IllegalArgumentException("targetId must not be blank");
        }
        JsonObject obj = envelope("duo_she_request");
        obj.addProperty("target_id", targetId);
        return obj.toString();
    }

    public static String encodeQiColorInspect(String observed) {
        if (observed == null || observed.isBlank()) {
            throw new IllegalArgumentException("observed must not be blank");
        }
        JsonObject obj = envelope("qi_color_inspect");
        obj.addProperty("observed", observed.trim());
        return obj.toString();
    }

    public static String encodeUseLifeCore(long instanceId) {
        if (instanceId < 0) {
            throw new IllegalArgumentException("instanceId must be >= 0, got " + instanceId);
        }
        JsonObject obj = envelope("use_life_core");
        obj.addProperty("instance_id", instanceId);
        return obj.toString();
    }

    // ─── 炼丹 (plan-alchemy-v1 §4) ──────────────────────────────────────────

    public static String encodeAlchemyOpenFurnace(BlockPos pos) {
        JsonObject obj = envelope("alchemy_open_furnace");
        addBlockPos(obj, pos);
        return obj.toString();
    }

    public static String encodeAlchemyTurnPage(int delta) {
        JsonObject obj = envelope("alchemy_turn_page");
        obj.addProperty("delta", delta);
        return obj.toString();
    }

    public static String encodeAlchemyLearnRecipe(String recipeId) {
        JsonObject obj = envelope("alchemy_learn_recipe");
        obj.addProperty("recipe_id", recipeId);
        return obj.toString();
    }

    public static String encodeAlchemyIgnite(BlockPos pos, String recipeId) {
        JsonObject obj = envelope("alchemy_ignite");
        addBlockPos(obj, pos);
        obj.addProperty("recipe_id", recipeId);
        return obj.toString();
    }

    public static String encodeAlchemyFeedSlot(BlockPos pos, int slotIdx, String material, int count) {
        JsonObject obj = envelope("alchemy_feed_slot");
        addBlockPos(obj, pos);
        obj.addProperty("slot_idx", slotIdx);
        obj.addProperty("material", material);
        obj.addProperty("count", count);
        return obj.toString();
    }

    public static String encodeAlchemyTakeBack(BlockPos pos, int slotIdx) {
        JsonObject obj = envelope("alchemy_take_back");
        addBlockPos(obj, pos);
        obj.addProperty("slot_idx", slotIdx);
        return obj.toString();
    }

    public static String encodeAlchemyInjectQi(BlockPos pos, double qi) {
        JsonObject obj = envelope("alchemy_intervention");
        addBlockPos(obj, pos);
        JsonObject inner = new JsonObject();
        inner.addProperty("kind", "inject_qi");
        inner.addProperty("qi", qi);
        obj.add("intervention", inner);
        return obj.toString();
    }

    public static String encodeAlchemyAdjustTemp(BlockPos pos, double temp) {
        JsonObject obj = envelope("alchemy_intervention");
        addBlockPos(obj, pos);
        JsonObject inner = new JsonObject();
        inner.addProperty("kind", "adjust_temp");
        inner.addProperty("temp", temp);
        obj.add("intervention", inner);
        return obj.toString();
    }

    public static String encodeAlchemyTakePill(String pillItemId) {
        JsonObject obj = envelope("alchemy_take_pill");
        obj.addProperty("pill_item_id", pillItemId);
        return obj.toString();
    }

    public static String encodeAlchemyFurnacePlace(BlockPos pos, long itemInstanceId) {
        JsonObject obj = envelope("alchemy_furnace_place");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        obj.addProperty("item_instance_id", itemInstanceId);
        return obj.toString();
    }

    public static String encodeCoffinOpen(BlockPos pos) {
        JsonObject obj = envelope("coffin_open");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        return obj.toString();
    }

    public static String encodeCoffinPlace(BlockPos pos, long itemInstanceId) {
        JsonObject obj = envelope("coffin_place");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        obj.addProperty("item_instance_id", itemInstanceId);
        return obj.toString();
    }

    public static String encodeCoffinEnter(BlockPos pos) {
        JsonObject obj = envelope("coffin_enter");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        return obj.toString();
    }

    public static String encodeCoffinLeave() {
        return envelope("coffin_leave").toString();
    }

    public sealed interface ApplyPillTarget {
        JsonObject toJson();
    }

    public enum SelfTarget implements ApplyPillTarget {
        INSTANCE;

        @Override
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "self");
            return o;
        }
    }

    public record MeridianTarget(MeridianId meridianId) implements ApplyPillTarget {
        @Override
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "meridian");
            o.addProperty("meridian_id", meridianId.name());
            return o;
        }
    }

    public static String encodeApplyPill(long instanceId, ApplyPillTarget target) {
        JsonObject obj = envelope("apply_pill");
        obj.addProperty("instance_id", instanceId);
        obj.add("target", target.toJson());
        return obj.toString();
    }

    public static String encodeApplyPillSelf(long instanceId) {
        return encodeApplyPill(instanceId, SelfTarget.INSTANCE);
    }

    public static String encodeLearnSkillScroll(long instanceId) {
        JsonObject obj = envelope("learn_skill_scroll");
        obj.addProperty("instance_id", instanceId);
        return obj.toString();
    }

    public static String encodeTechniqueScrollUse(long instanceId) {
        JsonObject obj = envelope("technique_scroll_use");
        obj.addProperty("instance_id", instanceId);
        return obj.toString();
    }

    // ─── Inventory move intent (client → server) ────────────────────────────

    /** 库存位置三态联合，匹配 server schema InventoryLocationV1。 */
    public sealed interface InvLocation {
        JsonObject toJson();
    }
    public record ContainerLoc(String containerId, int row, int col) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "container");
            o.addProperty("container_id", containerId);
            o.addProperty("row", row);
            o.addProperty("col", col);
            return o;
        }
    }
    public record EquipLoc(String slot) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "equip");
            o.addProperty("slot", slot);
            return o;
        }
    }
    public record HotbarLoc(int index) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "hotbar");
            o.addProperty("index", index);
            return o;
        }
    }

    public static String encodeInventoryMove(long instanceId, InvLocation from, InvLocation to) {
        JsonObject obj = envelope("inventory_move_intent");
        obj.addProperty("instance_id", instanceId);
        obj.add("from", from.toJson());
        obj.add("to", to.toJson());
        return obj.toString();
    }

    public static String encodeEquipFalseSkin(long itemInstanceId) {
        if (itemInstanceId < 0) {
            throw new IllegalArgumentException("itemInstanceId must be >= 0, got " + itemInstanceId);
        }
        JsonObject obj = envelope("equip_false_skin");
        obj.addProperty("slot", "false_skin");
        obj.addProperty("item_instance_id", itemInstanceId);
        return obj.toString();
    }

    public static String encodeForgeFalseSkin(FalseSkinKind kind) {
        if (kind == null) {
            throw new IllegalArgumentException("kind must not be null");
        }
        JsonObject obj = envelope("forge_false_skin");
        obj.addProperty("kind", kind.wireName());
        return obj.toString();
    }

    public static String encodePickupDroppedItem(long instanceId) {
        JsonObject obj = envelope("pickup_dropped_item");
        obj.addProperty("instance_id", instanceId);
        return obj.toString();
    }

    public static String encodeMineralProbe(int x, int y, int z) {
        JsonObject obj = envelope("mineral_probe");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        return obj.toString();
    }

    public static String encodeInventoryDiscardItem(long instanceId, InvLocation from) {
        JsonObject obj = envelope("inventory_discard_item");
        obj.addProperty("instance_id", instanceId);
        obj.add("from", from.toJson());
        return obj.toString();
    }

    public static String encodeDropWeapon(long instanceId, InvLocation from) {
        JsonObject obj = envelope("drop_weapon_intent");
        obj.addProperty("instance_id", instanceId);
        obj.add("from", from.toJson());
        return obj.toString();
    }

    public static String encodeRepairWeapon(long instanceId, int x, int y, int z) {
        JsonObject obj = envelope("repair_weapon_intent");
        obj.addProperty("instance_id", instanceId);
        com.google.gson.JsonArray pos = new com.google.gson.JsonArray();
        pos.add(x);
        pos.add(y);
        pos.add(z);
        obj.add("station_pos", pos);
        return obj.toString();
    }

    public static String encodeForgeStationPlace(int x, int y, int z, long itemInstanceId, int stationTier) {
        JsonObject obj = envelope("forge_station_place");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("item_instance_id", itemInstanceId);
        obj.addProperty("station_tier", stationTier);
        return obj.toString();
    }

    public static String encodeSpiritNichePlace(int x, int y, int z, long itemInstanceId) {
        JsonObject obj = envelope("spirit_niche_place");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("item_instance_id", itemInstanceId);
        return obj.toString();
    }

    public static String encodeSpiritNicheGaze(int x, int y, int z) {
        JsonObject obj = envelope("spirit_niche_gaze");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        return obj.toString();
    }

    public static String encodeSpiritNicheMarkCoordinate(int x, int y, int z) {
        JsonObject obj = envelope("spirit_niche_mark_coordinate");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        return obj.toString();
    }

    public static String encodeSpiritNicheActivateGuardian(
        int x,
        int y,
        int z,
        String guardianKind,
        java.util.List<String> materials
    ) {
        JsonObject obj = envelope("spirit_niche_activate_guardian");
        JsonArray nichePos = new JsonArray();
        nichePos.add(x);
        nichePos.add(y);
        nichePos.add(z);
        obj.add("niche_pos", nichePos);
        obj.addProperty("guardian_kind", guardianKind);
        JsonArray materialArray = new JsonArray();
        if (materials != null) {
            materials.stream()
                .filter(material -> material != null && !material.isBlank())
                .forEach(materialArray::add);
        }
        obj.add("materials", materialArray);
        return obj.toString();
    }

    public static String encodeZhenfaPlace(
        BlockPos pos,
        ZhenfaKind kind,
        ZhenfaCarrierKind carrier,
        double qiInvestRatio,
        String trigger
    ) {
        return encodeZhenfaPlace(pos, kind, carrier, qiInvestRatio, trigger, null, null);
    }

    public static String encodeZhenfaPlace(
        BlockPos pos,
        ZhenfaKind kind,
        ZhenfaCarrierKind carrier,
        double qiInvestRatio,
        String trigger,
        Long itemInstanceId,
        ZhenfaTargetFace targetFace
    ) {
        if (pos == null) {
            throw new IllegalArgumentException("pos must not be null");
        }
        if (kind == null) {
            throw new IllegalArgumentException("kind must not be null");
        }
        if (!Double.isFinite(qiInvestRatio) || qiInvestRatio < 0.0 || qiInvestRatio > 1.0) {
            throw new IllegalArgumentException("qiInvestRatio must be finite within [0, 1], got " + qiInvestRatio);
        }
        JsonObject obj = envelope("zhenfa_place");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        obj.addProperty("kind", kind.wireName());
        if (carrier != null) {
            obj.addProperty("carrier", carrier.wireName());
        }
        obj.addProperty("qi_invest_ratio", qiInvestRatio);
        if (trigger != null && !trigger.isBlank()) {
            obj.addProperty("trigger", trigger.trim());
        }
        if (itemInstanceId != null) {
            if (itemInstanceId < 0) {
                throw new IllegalArgumentException("itemInstanceId must be >= 0, got " + itemInstanceId);
            }
            obj.addProperty("item_instance_id", itemInstanceId.longValue());
        }
        if (targetFace != null) {
            obj.addProperty("target_face", targetFace.wireName());
        }
        return obj.toString();
    }

    public static String encodeZhenfaTrigger(Long instanceId) {
        JsonObject obj = envelope("zhenfa_trigger");
        if (instanceId != null) {
            if (instanceId < 0) {
                throw new IllegalArgumentException("instanceId must be >= 0, got " + instanceId);
            }
            obj.addProperty("instance_id", instanceId.longValue());
        }
        return obj.toString();
    }

    public static String encodeZhenfaDisarm(BlockPos pos, ZhenfaDisarmMode mode) {
        if (pos == null) {
            throw new IllegalArgumentException("pos must not be null");
        }
        if (mode == null) {
            throw new IllegalArgumentException("mode must not be null");
        }
        JsonObject obj = envelope("zhenfa_disarm");
        obj.addProperty("x", pos.getX());
        obj.addProperty("y", pos.getY());
        obj.addProperty("z", pos.getZ());
        obj.addProperty("mode", mode.wireName());
        return obj.toString();
    }

    public static String encodeSparringInviteResponse(String inviteId, boolean accepted, boolean timedOut) {
        if (inviteId == null || inviteId.isBlank()) {
            throw new IllegalArgumentException("inviteId must not be blank");
        }
        JsonObject obj = envelope("sparring_invite_response");
        obj.addProperty("invite_id", inviteId);
        obj.addProperty("accepted", accepted);
        obj.addProperty("timed_out", timedOut);
        return obj.toString();
    }

    public static String encodeTradeOfferRequest(String target, long offeredInstanceId) {
        if (target == null || target.isBlank()) {
            throw new IllegalArgumentException("target must not be blank");
        }
        if (offeredInstanceId < 0) {
            throw new IllegalArgumentException("offeredInstanceId must be >= 0, got " + offeredInstanceId);
        }
        JsonObject obj = envelope("trade_offer_request");
        obj.addProperty("target", target.trim());
        obj.addProperty("offered_instance_id", offeredInstanceId);
        return obj.toString();
    }

    public static String encodeTradeOfferResponse(String offerId, boolean accepted, Long requestedInstanceId) {
        if (offerId == null || offerId.isBlank()) {
            throw new IllegalArgumentException("offerId must not be blank");
        }
        JsonObject obj = envelope("trade_offer_response");
        obj.addProperty("offer_id", offerId);
        obj.addProperty("accepted", accepted);
        if (requestedInstanceId != null) {
            if (requestedInstanceId < 0) {
                throw new IllegalArgumentException("requestedInstanceId must be >= 0, got " + requestedInstanceId);
            }
            obj.addProperty("requested_instance_id", requestedInstanceId.longValue());
        }
        return obj.toString();
    }

    public static String encodeNpcInspectRequest(int npcEntityId) {
        if (npcEntityId < 0) {
            throw new IllegalArgumentException("npcEntityId must be >= 0, got " + npcEntityId);
        }
        JsonObject obj = envelope("npc_inspect_request");
        obj.addProperty("npc_entity_id", npcEntityId);
        return obj.toString();
    }

    public static String encodeNpcDialogueChoice(int npcEntityId, String optionId) {
        if (npcEntityId < 0) {
            throw new IllegalArgumentException("npcEntityId must be >= 0, got " + npcEntityId);
        }
        if (optionId == null || optionId.isBlank()) {
            throw new IllegalArgumentException("optionId must not be blank");
        }
        JsonObject obj = envelope("npc_dialogue_choice");
        obj.addProperty("npc_entity_id", npcEntityId);
        obj.addProperty("option_id", optionId.trim());
        return obj.toString();
    }

    public static String encodeNpcTradeRequest(int npcEntityId, List<Long> offeredItems, String requestedItemId) {
        if (npcEntityId < 0) {
            throw new IllegalArgumentException("npcEntityId must be >= 0, got " + npcEntityId);
        }
        if (requestedItemId == null || requestedItemId.isBlank()) {
            throw new IllegalArgumentException("requestedItemId must not be blank");
        }
        JsonObject obj = envelope("npc_trade_request");
        obj.addProperty("npc_entity_id", npcEntityId);
        JsonArray offered = new JsonArray();
        if (offeredItems != null) {
            for (Long item : offeredItems) {
                if (item == null || item < 0) {
                    throw new IllegalArgumentException("offeredItems must contain only non-negative ids");
                }
                offered.add(item.longValue());
            }
        }
        obj.add("offered_items", offered);
        obj.addProperty("requested_item_id", requestedItemId.trim());
        return obj.toString();
    }

    public static String encodeForgeTemperingHit(long sessionId, TemperBeat beat, int ticksRemaining) {
        if (sessionId < 0) {
            throw new IllegalArgumentException("sessionId must be >= 0, got " + sessionId);
        }
        if (beat == null) {
            throw new IllegalArgumentException("beat must not be null");
        }
        if (ticksRemaining < 0) {
            throw new IllegalArgumentException("ticksRemaining must be >= 0, got " + ticksRemaining);
        }
        JsonObject obj = envelope("forge_tempering_hit");
        obj.addProperty("session_id", sessionId);
        obj.addProperty("beat", beat.name());
        obj.addProperty("ticks_remaining", ticksRemaining);
        return obj.toString();
    }

    public static String encodeForgeInscriptionScroll(long sessionId, String inscriptionId) {
        if (sessionId < 0) {
            throw new IllegalArgumentException("sessionId must be >= 0, got " + sessionId);
        }
        if (inscriptionId == null || inscriptionId.isBlank()) {
            throw new IllegalArgumentException("inscriptionId must not be blank");
        }
        JsonObject obj = envelope("forge_inscription_scroll");
        obj.addProperty("session_id", sessionId);
        obj.addProperty("inscription_id", inscriptionId.trim());
        return obj.toString();
    }

    public static String encodeForgeConsecrationInject(long sessionId, double qiAmount) {
        if (sessionId < 0) {
            throw new IllegalArgumentException("sessionId must be >= 0, got " + sessionId);
        }
        if (!Double.isFinite(qiAmount) || qiAmount < 0.0) {
            throw new IllegalArgumentException("qiAmount must be finite and >= 0, got " + qiAmount);
        }
        JsonObject obj = envelope("forge_consecration_inject");
        obj.addProperty("session_id", sessionId);
        obj.addProperty("qi_amount", qiAmount);
        return obj.toString();
    }

    // ─── HUD combat intents (plan-HUD-v1 §11.3) ─────────────────────────────

    public static String encodeUseQuickSlot(int slot) {
        JsonObject obj = envelope("use_quick_slot");
        obj.addProperty("slot", slot);
        return obj.toString();
    }

    public static String encodeSelfAntidote(long instanceId) {
        if (instanceId < 0) {
            throw new IllegalArgumentException("instanceId must be >= 0, got " + instanceId);
        }
        JsonObject obj = envelope("self_antidote");
        obj.addProperty("instance_id", instanceId);
        return obj.toString();
    }

    /** itemId == null → 清空槽位。 */
    public static String encodeQuickSlotBind(int slot, String itemId) {
        JsonObject obj = envelope("quick_slot_bind");
        obj.addProperty("slot", slot);
        if (itemId == null || itemId.isEmpty()) {
            obj.add("item_id", com.google.gson.JsonNull.INSTANCE);
        } else {
            obj.addProperty("item_id", itemId);
        }
        return obj.toString();
    }

    public static String encodeSkillBarCast(int slot) {
        return encodeSkillBarCast(slot, null);
    }

    public static String encodeSkillBarCast(int slot, String target) {
        JsonObject obj = envelope("skill_bar_cast");
        obj.addProperty("slot", slot);
        if (target != null && !target.isBlank()) {
            obj.addProperty("target", target.trim());
        }
        return obj.toString();
    }

    public static String encodeSkillBarBindClear(int slot) {
        JsonObject obj = envelope("skill_bar_bind");
        obj.addProperty("slot", slot);
        obj.add("binding", com.google.gson.JsonNull.INSTANCE);
        return obj.toString();
    }

    public static String encodeSkillBarBindSkill(int slot, String skillId) {
        if (skillId == null || skillId.isBlank()) {
            throw new IllegalArgumentException("skillId must not be blank");
        }
        JsonObject obj = envelope("skill_bar_bind");
        obj.addProperty("slot", slot);
        JsonObject binding = new JsonObject();
        binding.addProperty("kind", "skill");
        binding.addProperty("skill_id", skillId);
        obj.add("binding", binding);
        return obj.toString();
    }

    public static String encodeSkillBarBindItem(int slot, String templateId) {
        if (templateId == null || templateId.isBlank()) {
            throw new IllegalArgumentException("templateId must not be blank");
        }
        JsonObject obj = envelope("skill_bar_bind");
        obj.addProperty("slot", slot);
        JsonObject binding = new JsonObject();
        binding.addProperty("kind", "item");
        binding.addProperty("template_id", templateId);
        obj.add("binding", binding);
        return obj.toString();
    }

    public static String encodeSkillConfigIntent(String skillId, JsonObject config) {
        if (skillId == null || skillId.isBlank()) {
            throw new IllegalArgumentException("skillId must not be blank");
        }
        if (config == null) {
            throw new IllegalArgumentException("config must not be null");
        }
        JsonObject obj = envelope("skill_config_intent");
        obj.addProperty("skill_id", skillId.trim());
        obj.add("config", config.deepCopy());
        return obj.toString();
    }

    public static String encodeChargeCarrier(String slot, double qiTarget) {
        if (!Double.isFinite(qiTarget) || qiTarget < 0.0 || qiTarget > 80.0) {
            throw new IllegalArgumentException("qiTarget must be finite in [0,80], got " + qiTarget);
        }
        JsonObject obj = envelope("charge_carrier");
        if (slot != null && !slot.isBlank()) {
            obj.addProperty("slot", slot.trim());
        }
        obj.addProperty("qi_target", qiTarget);
        return obj.toString();
    }

    public static String encodeThrowCarrier(String slot, double x, double y, double z, double power) {
        if (slot == null || slot.isBlank()) {
            throw new IllegalArgumentException("slot must not be blank");
        }
        if (!Double.isFinite(x) || !Double.isFinite(y) || !Double.isFinite(z)) {
            throw new IllegalArgumentException("dir vector must be finite");
        }
        if (!Double.isFinite(power) || power < 0.0 || power > 1.0) {
            throw new IllegalArgumentException("power must be finite in [0,1], got " + power);
        }
        JsonObject obj = envelope("throw_carrier");
        obj.addProperty("slot", slot.trim());
        com.google.gson.JsonArray dir = new com.google.gson.JsonArray();
        dir.add(x);
        dir.add(y);
        dir.add(z);
        obj.add("dir_unit", dir);
        obj.addProperty("power", power);
        return obj.toString();
    }

    public static String encodeAnqiContainerSwitch() {
        return envelope("anqi_container_switch").toString();
    }

    public static String encodeAnqiContainerSwitch(AnqiContainerKind to) {
        if (to == null) {
            return encodeAnqiContainerSwitch();
        }
        if (to == AnqiContainerKind.FENGLINGHE) {
            throw new IllegalArgumentException("fenglinghe cannot be switched during combat");
        }
        JsonObject obj = envelope("anqi_container_switch");
        obj.addProperty("to", to.wireName());
        return obj.toString();
    }

    public static String encodeJiemai() {
        return envelope("jiemai").toString();
    }

    public static String encodeMovementAction(MovementAction action) {
        if (action == null) {
            throw new IllegalArgumentException("movement action must not be null");
        }
        JsonObject obj = envelope("movement_action");
        obj.addProperty("action", action.wireName());
        return obj.toString();
    }

    public static String encodeStartExtractRequest(long portalEntityId) {
        JsonObject obj = envelope("start_extract_request");
        obj.addProperty("portal_entity_id", portalEntityId);
        return obj.toString();
    }

    public static String encodeCancelExtractRequest() {
        return envelope("cancel_extract_request").toString();
    }

    public static String encodeStartSearch(long containerEntityId) {
        if (containerEntityId < 0) {
            throw new IllegalArgumentException("containerEntityId must be >= 0, got " + containerEntityId);
        }
        JsonObject obj = envelope("start_search");
        obj.addProperty("container_entity_id", containerEntityId);
        return obj.toString();
    }

    public static String encodeCancelSearch() {
        return envelope("cancel_search").toString();
    }

    // ─── 灵田（plan-lingtian-v1 §1.2-§1.7） ──────────────────────────

    /** plan §1.2.2 — 起开垦 session。{@code mode} = "manual" | "auto"。 */
    public static String encodeLingtianStartTill(int x, int y, int z, long hoeInstanceId, String mode) {
        JsonObject obj = envelope("lingtian_start_till");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("hoe_instance_id", hoeInstanceId);
        obj.addProperty("mode", mode);
        return obj.toString();
    }

    /** plan §1.6 — 起翻新 session。 */
    public static String encodeLingtianStartRenew(int x, int y, int z, long hoeInstanceId) {
        JsonObject obj = envelope("lingtian_start_renew");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("hoe_instance_id", hoeInstanceId);
        return obj.toString();
    }

    /** plan §1.2.3 — 起种植 session（背包内须有该 plant_id 的种子）。 */
    public static String encodeLingtianStartPlanting(int x, int y, int z, String plantId) {
        JsonObject obj = envelope("lingtian_start_planting");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("plant_id", plantId);
        return obj.toString();
    }

    /** plan §1.5 — 起收获 session。{@code mode} = "manual" | "auto"。 */
    public static String encodeLingtianStartHarvest(int x, int y, int z, String mode) {
        JsonObject obj = envelope("lingtian_start_harvest");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("mode", mode);
        return obj.toString();
    }

    /** plan §1.4 + plan-alchemy-recycle-v1 — 起补灵 session。 */
    public static String encodeLingtianStartReplenish(int x, int y, int z, String source) {
        JsonObject obj = envelope("lingtian_start_replenish");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        obj.addProperty("source", source);
        return obj.toString();
    }

    /** plan §1.7 — 起偷灵 session。 */
    public static String encodeLingtianStartDrainQi(int x, int y, int z) {
        JsonObject obj = envelope("lingtian_start_drain_qi");
        obj.addProperty("x", x);
        obj.addProperty("y", y);
        obj.addProperty("z", z);
        return obj.toString();
    }

    // ─── 通用手搓 (plan-craft-v1 P2) ────────────────────────────────────────

    /** plan-craft-v1 §2 — 玩家点 [开始手搓]。recipe_id 为 server `RecipeId.as_str()`。 */
    public static String encodeCraftStart(String recipeId) {
        return encodeCraftStart(recipeId, 1);
    }

    /** plan-craft-ux-v1 P2 — 批量制作数量。server 端低版本会按 1 处理。 */
    public static String encodeCraftStart(String recipeId, int quantity) {
        if (recipeId == null || recipeId.isEmpty()) {
            throw new IllegalArgumentException("recipeId must not be empty");
        }
        if (quantity < 1) {
            throw new IllegalArgumentException("quantity must be >= 1");
        }
        if (quantity > MAX_CRAFT_QUANTITY) {
            throw new IllegalArgumentException("quantity must be <= " + MAX_CRAFT_QUANTITY);
        }
        JsonObject obj = envelope("craft_start");
        obj.addProperty("recipe_id", recipeId);
        obj.addProperty("quantity", quantity);
        return obj.toString();
    }

    /** plan-craft-v1 §5 决策门 #3 — 取消进行中的 session（70% 材料返还，qi 不退）。 */
    public static String encodeCraftCancel() {
        return envelope("craft_cancel").toString();
    }

    /** 通用请求编码（combat UI 系列使用）。payload 可为 {@code null}。 */
    public static String encodeGeneric(String type, JsonObject payload) {
        JsonObject obj = envelope(type);
        if (payload != null) {
            for (String key : payload.keySet()) {
                obj.add(key, payload.get(key));
            }
        }
        return obj.toString();
    }

    private static JsonObject envelope(String type) {
        JsonObject obj = new JsonObject();
        obj.addProperty("type", type);
        obj.addProperty("v", VERSION);
        return obj;
    }

    private static JsonObject voidActionRequest(VoidActionKind kind) {
        JsonObject request = new JsonObject();
        request.addProperty("kind", kind.wireName());
        return request;
    }

    private static String encodeVoidAction(JsonObject request) {
        JsonObject obj = envelope("void_action");
        obj.add("request", request);
        return obj.toString();
    }

    private static String requireNonBlank(String value, String field) {
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(field + " must not be blank");
        }
        return value.trim();
    }

    private static void addBlockPos(JsonObject obj, BlockPos pos) {
        JsonArray arr = new JsonArray();
        arr.add(pos.getX());
        arr.add(pos.getY());
        arr.add(pos.getZ());
        obj.add("furnace_pos", arr);
    }
}
