package com.bong.client.daozhan;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-daozhan-v1 P1 — DaoZhanDisguiseHandler 契约测试。
 *
 * <p>测契约（可观察行为）不测实现：
 * <ul>
 *   <li>happy path：正常 enter/reveal payload 正确更新 disguised 集合
 *   <li>边界：空 entity_ids 数组、v 不匹配、type 不匹配、损坏 JSON、null payload、负数 size
 *   <li>状态转换：enter 全量替换 → reveal 增量移除
 *   <li>disconnect 清空状态
 *   <li>wire name pin：channel 名变更时撞红（须同步更新 server Rust）
 * </ul>
 */
class DaoZhanDisguiseHandlerTest {

    @BeforeEach
    void clearState() {
        DaoZhanDisguiseHandler.clearOnDisconnect();
    }

    // ── happy path ─────────────────────────────────────────────────────────

    @Test
    void enter_payload_marks_entities_as_disguised() {
        String payload = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,77]}
                """;
        boolean handled = DaoZhanDisguiseHandler.handleEnter(payload, payload.getBytes().length);
        assertTrue(handled, "有效 enter payload 应返回 true");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(42), "entity 42 应为 Disguised（Mimicry 态道伥）");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(77), "entity 77 应为 Disguised");
        assertFalse(DaoZhanDisguiseHandler.isDisguised(99), "未在列表中的 entity 99 不应为 Disguised");
    }

    @Test
    void reveal_payload_removes_entities_from_disguised() {
        // 先 enter
        String enter = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,77,88]}
                """;
        DaoZhanDisguiseHandler.handleEnter(enter, enter.getBytes().length);

        // reveal 移除 42（道伥暴起）
        String reveal = """
                {"v":1,"type":"daozhan_reveal","entity_ids":[42]}
                """;
        boolean handled = DaoZhanDisguiseHandler.handleReveal(reveal, reveal.getBytes().length);
        assertTrue(handled, "有效 reveal payload 应返回 true");
        assertFalse(DaoZhanDisguiseHandler.isDisguised(42), "暴起后 entity 42 不应再是 Disguised（Mimicry→Ambush）");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(77), "未暴起的 entity 77 仍应为 Disguised");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(88), "未暴起的 entity 88 仍应为 Disguised");
    }

    @Test
    void enter_replaces_all_previous_state() {
        // 第一次 enter：42, 77
        String first = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,77]}
                """;
        DaoZhanDisguiseHandler.handleEnter(first, first.getBytes().length);

        // 第二次 enter：只有 99（全量替换 — server 定期 sync）
        String second = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[99]}
                """;
        DaoZhanDisguiseHandler.handleEnter(second, second.getBytes().length);

        assertFalse(DaoZhanDisguiseHandler.isDisguised(42), "全量替换后 42 不应再 Disguised（旧状态清除）");
        assertFalse(DaoZhanDisguiseHandler.isDisguised(77), "全量替换后 77 不应再 Disguised");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(99), "全量替换后 99 应为 Disguised（新道伥加入）");
    }

    // ── 边界 ───────────────────────────────────────────────────────────────

    @Test
    void enter_with_empty_entity_ids_clears_state() {
        // 先设置一些 disguised 道伥
        String setup = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,77]}
                """;
        DaoZhanDisguiseHandler.handleEnter(setup, setup.getBytes().length);

        // 空列表 enter = server 说当前无 Mimicry 态道伥
        String empty = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[]}
                """;
        boolean handled = DaoZhanDisguiseHandler.handleEnter(empty, empty.getBytes().length);
        assertTrue(handled, "空 entity_ids enter 应有效处理（服务端宣布全部暴起/死亡）");
        assertTrue(
            DaoZhanDisguiseHandler.disguisedEntityIdsSnapshot().isEmpty(),
            "空列表 enter 后 disguised 集合应清空（期望 size=0，实际 "
                + DaoZhanDisguiseHandler.disguisedEntityIdsSnapshot().size() + "）"
        );
    }

    @Test
    void reveal_with_empty_entity_ids_is_no_op() {
        String setup = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42]}
                """;
        DaoZhanDisguiseHandler.handleEnter(setup, setup.getBytes().length);

        String reveal = """
                {"v":1,"type":"daozhan_reveal","entity_ids":[]}
                """;
        boolean handled = DaoZhanDisguiseHandler.handleReveal(reveal, reveal.getBytes().length);
        assertTrue(handled, "空 entity_ids reveal 应有效处理（no-op）");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(42), "空 reveal 不应影响现有 disguised 道伥");
    }

    @Test
    void null_payload_returns_false() {
        assertFalse(
            DaoZhanDisguiseHandler.handleEnter(null, 0),
            "null payload 应返回 false"
        );
        assertFalse(
            DaoZhanDisguiseHandler.handleReveal(null, 0),
            "reveal null payload 应返回 false"
        );
    }

    @Test
    void negative_size_returns_false() {
        String payload = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[]}
                """;
        assertFalse(
            DaoZhanDisguiseHandler.handleEnter(payload, -1),
            "payloadSizeBytes < 0 应返回 false（防御过大 payload）"
        );
    }

    @Test
    void wrong_version_returns_false() {
        // v != 1 的 payload 应被拒绝（协议版本不匹配）
        String payload = """
                {"v":2,"type":"daozhan_disguise_enter","entity_ids":[42]}
                """;
        assertFalse(
            DaoZhanDisguiseHandler.handleEnter(payload, payload.getBytes().length),
            "版本号 v=2 应被拒绝（期望 v=1，协议升级时须同步改双端）"
        );
        assertFalse(DaoZhanDisguiseHandler.isDisguised(42), "拒绝后状态不应被修改");
    }

    @Test
    void wrong_type_field_for_enter_returns_false() {
        // enter channel 收到 reveal type
        String payload = """
                {"v":1,"type":"daozhan_reveal","entity_ids":[42]}
                """;
        assertFalse(
            DaoZhanDisguiseHandler.handleEnter(payload, payload.getBytes().length),
            "enter handler 收到 reveal type 应返回 false（type 守卫）"
        );
    }

    @Test
    void wrong_type_field_for_reveal_returns_false() {
        String setup = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42]}
                """;
        DaoZhanDisguiseHandler.handleEnter(setup, setup.getBytes().length);

        // reveal channel 收到 enter type
        String payload = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42]}
                """;
        assertFalse(
            DaoZhanDisguiseHandler.handleReveal(payload, payload.getBytes().length),
            "reveal handler 收到 enter type 应返回 false"
        );
        // 状态不应因 wrong-type call 被清空
        assertTrue(DaoZhanDisguiseHandler.isDisguised(42), "wrong-type reveal call 不应清空 disguised 状态");
    }

    @Test
    void corrupted_json_returns_false() {
        String corrupt = "{not valid json";
        assertFalse(
            DaoZhanDisguiseHandler.handleEnter(corrupt, corrupt.getBytes().length),
            "损坏 JSON 应返回 false，不应抛异常（防御性解析）"
        );
    }

    @Test
    void entity_ids_with_null_items_are_skipped() {
        // 混有 null 的数组（server 不应发但我们要防御）
        String payload = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[42,null,77]}
                """;
        boolean handled = DaoZhanDisguiseHandler.handleEnter(payload, payload.getBytes().length);
        assertTrue(handled, "含 null 项的数组应能处理（跳过 null，取有效 id）");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(42), "有效 id 42 应被 disguised");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(77), "有效 id 77 应被 disguised");
    }

    // ── 断线清理 ───────────────────────────────────────────────────────────

    @Test
    void clear_on_disconnect_removes_all_disguised() {
        String setup = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[1,2,3]}
                """;
        DaoZhanDisguiseHandler.handleEnter(setup, setup.getBytes().length);

        DaoZhanDisguiseHandler.clearOnDisconnect();

        assertTrue(
            DaoZhanDisguiseHandler.disguisedEntityIdsSnapshot().isEmpty(),
            "断线后 disguised 集合应清空（防跨 session 状态泄漏）"
        );
        assertFalse(DaoZhanDisguiseHandler.isDisguised(1), "断线后 entity 1 不应为 Disguised");
    }

    // ── wire name pin 测试 ─────────────────────────────────────────────────

    @Test
    void wire_name_daozhan_disguise_enter_is_stable() {
        // type wire name 变更时此测试撞红 → 须同步更新 server 侧 DaoZhanDisguiseS2c.disguise_enter()
        String payload = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[]}
                """;
        assertTrue(
            DaoZhanDisguiseHandler.handleEnter(payload, payload.getBytes().length),
            "wire name daozhan_disguise_enter 必须稳定（改变时须同步更新 server Rust + client Java）"
        );
    }

    @Test
    void wire_name_daozhan_reveal_is_stable() {
        String payload = """
                {"v":1,"type":"daozhan_reveal","entity_ids":[]}
                """;
        assertTrue(
            DaoZhanDisguiseHandler.handleReveal(payload, payload.getBytes().length),
            "wire name daozhan_reveal 必须稳定（改变时须同步更新 server Rust + client Java）"
        );
    }

    // ── snapshot 测试 ──────────────────────────────────────────────────────

    @Test
    void snapshot_reflects_current_state() {
        String setup = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[10,20]}
                """;
        DaoZhanDisguiseHandler.handleEnter(setup, setup.getBytes().length);

        List<Integer> snap = DaoZhanDisguiseHandler.disguisedEntityIdsSnapshot();
        assertEquals(2, snap.size(), "snapshot 大小应为 2（ids=[10,20]）");
        assertTrue(snap.contains(10), "snapshot 应含 id 10");
        assertTrue(snap.contains(20), "snapshot 应含 id 20");
    }

    @Test
    void reveal_multiple_entities_at_once() {
        // 多个道伥同时暴起（群体触发场景）
        String enter = """
                {"v":1,"type":"daozhan_disguise_enter","entity_ids":[1,2,3,4,5]}
                """;
        DaoZhanDisguiseHandler.handleEnter(enter, enter.getBytes().length);

        String reveal = """
                {"v":1,"type":"daozhan_reveal","entity_ids":[2,4]}
                """;
        DaoZhanDisguiseHandler.handleReveal(reveal, reveal.getBytes().length);

        assertFalse(DaoZhanDisguiseHandler.isDisguised(2), "entity 2 应被移除（暴起）");
        assertFalse(DaoZhanDisguiseHandler.isDisguised(4), "entity 4 应被移除（暴起）");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(1), "未暴起的 entity 1 仍应 disguised");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(3), "未暴起的 entity 3 仍应 disguised");
        assertTrue(DaoZhanDisguiseHandler.isDisguised(5), "未暴起的 entity 5 仍应 disguised");
    }
}
