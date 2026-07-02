package com.bong.client.network;

import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-inventory-hint-panel-v1 P1 — {@link InventoryMoveRejectedHandler} 饱和测试。
 * <p>
 * 覆盖：① happy path（全部 24 个 wire tag 文案）② 边界（空 reason / 未知 reason / cap 缺省 /
 * 空 required_realm）③ 错误分支（缺字段 → noOp）④ 状态转换（500ms 去重窗口：命中/边界/过期/跨
 * reason 不互相影响）。断言可观察契约（toast 文案 / color / duration），不绑渲染内部。
 */
class InventoryMoveRejectedHandlerTest {

    private static final int WARNING_COLOR = 0xFFAA55;
    private static final long TOAST_DURATION_MILLIS = 3000L;
    private static final String PREFIX = "天道警示：";

    @BeforeEach
    void setUp() {
        BongToast.resetForTests();
        InventoryMoveRejectedHandler.resetForTests();
    }

    @AfterEach
    void tearDown() {
        BongToast.resetForTests();
        InventoryMoveRejectedHandler.resetForTests();
    }

    // ─── happy path：全部 24 个 wire tag 都有专属文案 ──────────────────

    private static final List<String> ALL_WIRE_TAGS = List.of(
        "from_location_mismatch",
        "instance_not_found",
        "unknown_container_id",
        "target_out_of_bounds",
        "target_occupied",
        "multi_overlap_not_supported",
        "hotbar_index_out_of_range",
        "hotbar_occupied",
        "swap_footprint_mismatch",
        "unknown_item_template",
        "worn_stack_not_top",
        "forbidden_in_hotbar",
        "pack_detached",
        "held_worn_mismatch",
        "equip_category_mismatch",
        "off_hand_type_mismatch",
        "hand_occupied",
        "two_handed_locks_other",
        "armor_durability_zero",
        "armor_slot_mismatch",
        "armor_slot_unresolvable",
        "pack_equip_slot_mismatch",
        "worn_cap_full",
        "realm_too_low"
    );

    @Test
    void allTwentyFourWireTagsProduceDistinctPrefixedMessages() {
        assertEquals(24, ALL_WIRE_TAGS.size(), "wire tag fixture list itself must stay in sync with server enum");

        List<String> messages = ALL_WIRE_TAGS.stream()
            .map(tag -> InventoryMoveRejectedHandler.messageFor(tag, "Condense", "chest", 3))
            .collect(Collectors.toList());

        for (int i = 0; i < ALL_WIRE_TAGS.size(); i++) {
            String message = messages.get(i);
            assertTrue(message.startsWith(PREFIX),
                "reason=" + ALL_WIRE_TAGS.get(i) + " message must carry \"天道警示：\" prefix, got: " + message);
            assertTrue(message.length() > PREFIX.length(),
                "reason=" + ALL_WIRE_TAGS.get(i) + " must have a non-empty body after the prefix");
        }

        Set<String> distinct = Set.copyOf(messages);
        assertEquals(ALL_WIRE_TAGS.size(), distinct.size(),
            "every reason must map to a distinct message; a duplicate means two reasons share copy-pasted text: " + messages);
    }

    @Test
    void handOccupiedMessage() {
        assertEquals(PREFIX + "该手已持械，请先卸下再更换",
            InventoryMoveRejectedHandler.messageFor("hand_occupied", null, null, null));
    }

    @Test
    void twoHandedLocksOtherMessage() {
        assertEquals(PREFIX + "双手兵器占用双手，另一侧已被锁定",
            InventoryMoveRejectedHandler.messageFor("two_handed_locks_other", null, null, null));
    }

    @Test
    void equipCategoryMismatchMessage() {
        assertEquals(PREFIX + "此物品类不符，无法装入该槽位",
            InventoryMoveRejectedHandler.messageFor("equip_category_mismatch", null, null, null));
    }

    @Test
    void armorDurabilityZeroMessage() {
        assertEquals(PREFIX + "此甲损毁殆尽，无法穿戴",
            InventoryMoveRejectedHandler.messageFor("armor_durability_zero", null, null, null));
    }

    // ─── realm_too_low：境界不足文案走 RealmLabel 转中文（plan §8.1 决议 #3） ──

    @Test
    void realmTooLowConvertsEnglishTagToChineseRealmName() {
        String message = InventoryMoveRejectedHandler.messageFor("realm_too_low", "Condense", null, null);
        assertTrue(message.contains("凝脉"), "must show Chinese realm name '凝脉', got: " + message);
        assertFalse(message.contains("Condense"), "must NOT leak the raw English wire tag to the player: " + message);
    }

    @Test
    void realmTooLowCoversAllSixCanonicalRealms() {
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Awaken", null, null).contains("醒灵"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Induce", null, null).contains("引气"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Condense", null, null).contains("凝脉"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Solidify", null, null).contains("固元"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Spirit", null, null).contains("通灵"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("realm_too_low", "Void", null, null).contains("化虚"));
    }

    @Test
    void realmTooLowWithNullRequiredRealmFallsBackToMortalLabel() {
        // Defensive: required_realm should always be present when reason=realm_too_low,
        // but the handler must not NPE/crash if it's missing.
        String message = InventoryMoveRejectedHandler.messageFor("realm_too_low", null, null, null);
        assertTrue(message.contains("凡体"), "RealmLabel.displayName(\"\") fallback is '凡体', got: " + message);
    }

    // ─── worn_cap_full：文案带 slot + cap 数值（plan §P1 明确点名） ──────

    @Test
    void wornCapFullIncludesSlotChineseNameAndCapNumber() {
        String message = InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "chest", 3);
        assertTrue(message.contains("胸部"), "must show Chinese slot name '胸部', got: " + message);
        assertTrue(message.contains("3"), "must show the cap number 3, got: " + message);
    }

    @Test
    void wornCapFullHeadAndFeetCapIsTwo() {
        assertTrue(InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "head", 2).contains("头部"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "head", 2).contains("2"));
        assertTrue(InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "feet", 2).contains("足部"));
    }

    @Test
    void wornCapFullLegsCapIsThree() {
        assertTrue(InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "legs", 3).contains("腿部"));
    }

    @Test
    void wornCapFullWithNullCapDoesNotCrashAndShowsPlaceholder() {
        String message = InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "chest", null);
        assertTrue(message.contains("?"), "missing cap must degrade to a visible placeholder, not silently omit: " + message);
    }

    @Test
    void wornCapFullWithUnknownSlotKeyPassesThroughRawKey() {
        String message = InventoryMoveRejectedHandler.messageFor("worn_cap_full", null, "mystery_slot", 1);
        assertTrue(message.contains("mystery_slot"), "unknown slot key must not be silently dropped: " + message);
    }

    // ─── armor_slot_mismatch / pack_equip_slot_mismatch：slot 后缀 ──────

    @Test
    void armorSlotMismatchIncludesExpectedSlotSuffix() {
        String message = InventoryMoveRejectedHandler.messageFor("armor_slot_mismatch", null, "legs", null);
        assertTrue(message.contains("腿部"), "expected: " + message);
    }

    @Test
    void packEquipSlotMismatchIncludesExpectedSlotSuffix() {
        String message = InventoryMoveRejectedHandler.messageFor("pack_equip_slot_mismatch", null, "feet", null);
        assertTrue(message.contains("足部"), "expected: " + message);
    }

    @Test
    void armorSlotMismatchWithNullSlotOmitsSuffixGracefully() {
        String message = InventoryMoveRejectedHandler.messageFor("armor_slot_mismatch", null, null, null);
        assertTrue(message.startsWith(PREFIX), "must still be a well-formed message: " + message);
        assertFalse(message.contains("null"), "must never leak the literal string 'null': " + message);
    }

    // ─── armor_slot_unresolvable：数据/注册表缺口（equip_slot_for_item_id 返回 None），
    //     区别于 armor_slot_mismatch（已知 expected_slot，只是穿错槽）；此 reason 从不
    //     携带 slot，绝不能让 "unknown" 占位符泄漏进中文文案 ─────────────

    @Test
    void armorSlotUnresolvableMessageDoesNotLeakUnknownPlaceholder() {
        String message = InventoryMoveRejectedHandler.messageFor("armor_slot_unresolvable", null, null, null);
        assertTrue(message.startsWith(PREFIX), "must still be a well-formed message: " + message);
        assertFalse(message.toLowerCase().contains("unknown"),
            "must never leak the raw server-side 'unknown' placeholder tag: " + message);
        assertFalse(message.contains("null"), "must never leak the literal string 'null': " + message);
    }

    @Test
    void armorSlotUnresolvableMessageIsDistinctFromArmorSlotMismatch() {
        String unresolvable = InventoryMoveRejectedHandler.messageFor("armor_slot_unresolvable", null, "chest", null);
        String mismatch = InventoryMoveRejectedHandler.messageFor("armor_slot_mismatch", null, "chest", null);
        assertFalse(unresolvable.equals(mismatch),
            "armor_slot_unresolvable (无法解析槽位) and armor_slot_mismatch (穿错已知槽位) "
                + "carry different semantics and must not collapse into the same copy: " + unresolvable);
    }

    @Test
    void armorSlotUnresolvableIgnoresAnyStraySlotField() {
        // server never sends `slot` alongside armor_slot_unresolvable (ArmorSlotUnresolvable::slot()
        // is always None), but the client copy must not depend on it being absent either way.
        String message = InventoryMoveRejectedHandler.messageFor("armor_slot_unresolvable", null, "chest", null);
        assertTrue(message.startsWith(PREFIX), "must still be a well-formed message: " + message);
    }

    // ─── slotDisplayName：全部映射 + 边界 ────────────────────────────

    @Test
    void slotDisplayNameMapsAllSixKnownSlots() {
        assertEquals("头部", InventoryMoveRejectedHandler.slotDisplayName("head"));
        assertEquals("胸部", InventoryMoveRejectedHandler.slotDisplayName("chest"));
        assertEquals("腿部", InventoryMoveRejectedHandler.slotDisplayName("legs"));
        assertEquals("足部", InventoryMoveRejectedHandler.slotDisplayName("feet"));
        assertEquals("主手", InventoryMoveRejectedHandler.slotDisplayName("main_hand"));
        assertEquals("副手", InventoryMoveRejectedHandler.slotDisplayName("off_hand"));
        assertEquals("余手", InventoryMoveRejectedHandler.slotDisplayName("extra_hand_0"));
        assertEquals("余手", InventoryMoveRejectedHandler.slotDisplayName("extra_hand_1"));
    }

    @Test
    void slotDisplayNameBoundaries() {
        assertEquals("", InventoryMoveRejectedHandler.slotDisplayName(null));
        assertEquals("", InventoryMoveRejectedHandler.slotDisplayName(""));
        assertEquals("", InventoryMoveRejectedHandler.slotDisplayName("   "));
        assertEquals("unknown_slot_key", InventoryMoveRejectedHandler.slotDisplayName("unknown_slot_key"));
    }

    // ─── unknown reason：兜底文案，不静默丢弃 ────────────────────────

    @Test
    void unknownReasonFallsBackToGenericRejectionMessage() {
        String message = InventoryMoveRejectedHandler.messageFor("some_future_reason_not_yet_wired", null, null, null);
        assertEquals(PREFIX + "此操作被天道拒绝", message);
    }

    // ─── route() 全链路契约测：JSON → handler → BongToast(String,int,long,long) ──

    @Test
    void routeHandlesInventoryMoveRejectedAndShowsToastWithCorrectColorAndDuration() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"hand_occupied\""
            + "}";

        long before = System.currentTimeMillis();
        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);
        long after = System.currentTimeMillis();

        assertFalse(result.isParseError(), "valid JSON must parse: " + result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());

        BongToast toast = BongToast.current(after);
        assertFalse(toast.isEmpty(), "toast must be shown for a rejected move");
        assertEquals(PREFIX + "该手已持械，请先卸下再更换", toast.text().getString());
        assertEquals(WARNING_COLOR, toast.color(), "must use the warning color, not narration SYSTEM_WARNING styling");
        assertTrue(toast.shownAtMillis() >= before && toast.shownAtMillis() <= after);
        assertEquals(TOAST_DURATION_MILLIS, toast.expiresAtMillis() - toast.shownAtMillis(),
            "toast duration must be exactly 3000ms per plan §P3");
    }

    @Test
    void routeHandlesRealmTooLowEndToEndWithChineseRealmInToast() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"realm_too_low\","
            + "\"required_realm\":\"Condense\""
            + "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), result.logMessage());
        String toastText = BongToast.current(System.currentTimeMillis()).text().getString();
        assertTrue(toastText.contains("凝脉"), "expected Chinese realm name in toast: " + toastText);
        assertFalse(toastText.contains("Condense"), "must not leak English tag to player: " + toastText);
    }

    @Test
    void routeHandlesWornCapFullEndToEndWithSlotAndCapNumbers() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"worn_cap_full\","
            + "\"slot\":\"chest\","
            + "\"cap\":3"
            + "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), result.logMessage());
        String toastText = BongToast.current(System.currentTimeMillis()).text().getString();
        assertTrue(toastText.contains("胸部") && toastText.contains("3"), "expected slot+cap in toast: " + toastText);
    }

    // ─── 错误分支：缺字段 → noOp，不崩溃、不显示空 toast ────────────

    @Test
    void routeMissingReasonFieldIsSafeNoOp() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\""
            + "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "missing optional field should still parse as valid JSON");
        assertTrue(result.isNoOp(), result.logMessage());
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(), "no toast should be shown");
    }

    @Test
    void routeBlankReasonFieldIsSafeNoOp() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"\""
            + "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), result.logMessage());
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty());
    }

    @Test
    void routeNonStringReasonFieldIsSafeNoOp() {
        String json = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":42"
            + "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), result.logMessage());
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty());
    }

    // ─── 状态转换：500ms 去重窗口（命中 / 边界 / 过期 / 跨 reason） ──

    @Test
    void isDuplicateWithinWindow_neverRecordedReasonIsNotDuplicate() {
        assertFalse(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L));
    }

    @Test
    void isDuplicateWithinWindow_zeroElapsedIsDuplicate() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        assertTrue(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L));
    }

    @Test
    void isDuplicateWithinWindow_justUnderWindowIsStillDuplicate() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        assertTrue(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L + 499L));
    }

    @Test
    void isDuplicateWithinWindow_exactlyAtWindowEdgeIsNoLongerDuplicate() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        assertFalse(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L + 500L),
            "500ms elapsed must exactly clear the window (off-by-one boundary)");
    }

    @Test
    void isDuplicateWithinWindow_afterWindowExpiresIsNotDuplicate() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        assertFalse(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L + 501L));
    }

    @Test
    void isDuplicateWithinWindow_differentReasonsTrackedIndependently() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        assertFalse(InventoryMoveRejectedHandler.isDuplicateWithinWindow("worn_cap_full", 1_000_000L),
            "dedupe must be keyed per-reason, not global");
    }

    @Test
    void resetForTestsClearsDedupeState() {
        InventoryMoveRejectedHandler.recordShown("realm_too_low", 1_000_000L);
        InventoryMoveRejectedHandler.resetForTests();
        assertFalse(InventoryMoveRejectedHandler.isDuplicateWithinWindow("realm_too_low", 1_000_000L));
    }

    @Test
    void routeSameReasonTwiceInQuickSuccessionSkipsSecondToast() {
        String firstJson = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"worn_cap_full\","
            + "\"slot\":\"chest\","
            + "\"cap\":3"
            + "}";
        String secondJson = "{"
            + "\"v\":1,"
            + "\"type\":\"inventory_move_rejected\","
            + "\"reason\":\"worn_cap_full\","
            + "\"slot\":\"legs\","
            + "\"cap\":3"
            + "}";

        ServerDataRouter router = ServerDataRouter.createDefault();
        ServerDataRouter.RouteResult first =
            router.route(firstJson, firstJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(first.isHandled(), first.logMessage());
        String firstToastText = BongToast.current(System.currentTimeMillis()).text().getString();
        assertTrue(firstToastText.contains("胸部"), firstToastText);

        // Same reason fired again well within the 500ms window (in-process calls are microseconds apart).
        ServerDataRouter.RouteResult second =
            router.route(secondJson, secondJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(second.isNoOp(), second.logMessage());

        // Toast must remain the FIRST message; the deduped second call must not overwrite it.
        String toastTextAfterDupe = BongToast.current(System.currentTimeMillis()).text().getString();
        assertEquals(firstToastText, toastTextAfterDupe,
            "deduped call must not overwrite the still-active toast with the second payload's slot");
    }
}
