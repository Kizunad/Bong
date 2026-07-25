package com.bong.client.fauna;

import com.bong.client.network.ServerDataEnvelope;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-devour-rat-model P2 —— {@link RatQiTierHandler} 契约测试。
 *
 * <p>锁住：wire 解析（含全部拒收分支）、全量替换语义、缺省 0 档、越界夹取、断线清理。
 */
class RatQiTierHandlerTest {

    @BeforeEach
    void resetState() {
        RatQiTierHandler.clearOnDisconnect();
    }

    private static boolean feed(String payload) {
        return RatQiTierHandler.handleSync(payload, payload.getBytes().length);
    }

    private static String sync(String entriesJson) {
        return "{\"v\":1,\"type\":\"rat_qi_tier\",\"entries\":" + entriesJson + "}";
    }

    // ── happy path ───────────────────────────────────────────────────────────

    @Test
    void valid_payload_populates_tiers() {
        assertTrue(feed(sync("[{\"id\":42,\"tier\":2},{\"id\":77,\"tier\":1}]")),
            "结构完整的 payload 应被接受");

        assertEquals(2, RatQiTierHandler.tierOf(42),
            "entity 42 服务端下发 tier=2，应查得 2（尾脊全亮）");
        assertEquals(1, RatQiTierHandler.tierOf(77),
            "entity 77 服务端下发 tier=1，应查得 1（尾脊半亮）");
    }

    @Test
    void unknown_entity_defaults_to_tier_zero() {
        feed(sync("[{\"id\":42,\"tier\":2}]"));
        assertEquals(RatQiTierHandler.TIER_DEFAULT, RatQiTierHandler.tierOf(999),
            "名单外的鼠应按缺省 0 档渲染（服务端只下发 tier>0，缺席即 0）");
    }

    @Test
    void empty_entries_is_valid_and_clears_all() {
        feed(sync("[{\"id\":42,\"tier\":2}]"));
        assertTrue(feed(sync("[]")), "空名单是合法 keepalive，不是解析失败");
        assertEquals(RatQiTierHandler.TIER_DEFAULT, RatQiTierHandler.tierOf(42),
            "空表全量替换后所有鼠回 0 档（鼠已死 / 离开视野 / 储量归还 zone 的降档路径）");
        assertTrue(RatQiTierHandler.snapshot().isEmpty(), "档位表应被清空");
    }

    // ── 全量替换语义 ─────────────────────────────────────────────────────────

    @Test
    void sync_fully_replaces_previous_table() {
        feed(sync("[{\"id\":1,\"tier\":2},{\"id\":2,\"tier\":1}]"));
        feed(sync("[{\"id\":2,\"tier\":2},{\"id\":3,\"tier\":1}]"));

        assertEquals(0, RatQiTierHandler.tierOf(1),
            "上一轮有、这一轮缺席的 entity 1 必须降回 0 档，否则 stale 条目会让死鼠一直亮着");
        assertEquals(2, RatQiTierHandler.tierOf(2), "entity 2 的档位应被新值覆盖（1→2）");
        assertEquals(1, RatQiTierHandler.tierOf(3), "entity 3 是本轮新增");
        assertEquals(Map.of(2, 2, 3, 1), RatQiTierHandler.snapshot(),
            "全量替换后表内容应恰好等于本轮 payload");
    }

    @Test
    void tier_can_be_downgraded_by_later_sync() {
        feed(sync("[{\"id\":5,\"tier\":2}]"));
        feed(sync("[{\"id\":5,\"tier\":1}]"));
        assertEquals(1, RatQiTierHandler.tierOf(5),
            "降档（2→1）必须生效——只升不降会让尾脊永远停在最亮");
    }

    // ── 越界 / 脏值 ──────────────────────────────────────────────────────────

    @Test
    void tier_above_max_is_clamped() {
        feed(sync("[{\"id\":9,\"tier\":7}]"));
        assertEquals(RatQiTierHandler.TIER_MAX, RatQiTierHandler.tierOf(9),
            "越上界档位必须夹到 TIER_MAX，否则会去找磁盘上不存在的 devour_rat_q7.png");
    }

    @Test
    void negative_and_zero_tier_entries_are_not_stored() {
        feed(sync("[{\"id\":9,\"tier\":-3},{\"id\":10,\"tier\":0},{\"id\":11,\"tier\":2}]"));
        assertEquals(0, RatQiTierHandler.tierOf(9), "负档位夹到 0，且 0 档不占表");
        assertEquals(0, RatQiTierHandler.tierOf(10), "显式 0 档不占表（与「缺省即 0」约定一致）");
        assertEquals(Map.of(11, 2), RatQiTierHandler.snapshot(),
            "只有 tier>0 的条目进表，脏条目不得撑大档位表");
    }

    @Test
    void entry_missing_tier_field_defaults_to_zero_and_is_dropped() {
        feed(sync("[{\"id\":12}]"));
        assertEquals(0, RatQiTierHandler.tierOf(12), "缺 tier 字段按 0 档处理");
        assertTrue(RatQiTierHandler.snapshot().isEmpty(), "0 档不占表");
    }

    @Test
    void entry_missing_id_field_is_skipped_without_failing_payload() {
        assertTrue(feed(sync("[{\"tier\":2},{\"id\":13,\"tier\":1}]")),
            "个别条目缺 id 不应让整条 payload 作废");
        assertEquals(Map.of(13, 1), RatQiTierHandler.snapshot(),
            "缺 id 的条目跳过，其余条目照常生效");
    }

    @Test
    void malformed_entry_types_are_skipped() {
        assertTrue(feed(sync("[42,null,\"x\",{\"id\":14,\"tier\":2}]")),
            "数组里混入非对象条目应跳过而非整条拒收");
        assertEquals(Map.of(14, 2), RatQiTierHandler.snapshot());
    }

    @Test
    void non_numeric_tier_falls_back_to_zero() {
        assertTrue(feed(sync("[{\"id\":15,\"tier\":\"high\"}]")));
        assertEquals(0, RatQiTierHandler.tierOf(15), "非数字 tier 应回落 0 档而非抛异常");
    }

    @Test
    void clamp_tier_covers_boundaries() {
        assertEquals(0, RatQiTierHandler.clampTier(Integer.MIN_VALUE));
        assertEquals(0, RatQiTierHandler.clampTier(-1));
        assertEquals(0, RatQiTierHandler.clampTier(0));
        assertEquals(1, RatQiTierHandler.clampTier(1));
        assertEquals(RatQiTierHandler.TIER_MAX, RatQiTierHandler.clampTier(RatQiTierHandler.TIER_MAX));
        assertEquals(RatQiTierHandler.TIER_MAX, RatQiTierHandler.clampTier(RatQiTierHandler.TIER_MAX + 1));
        assertEquals(RatQiTierHandler.TIER_MAX, RatQiTierHandler.clampTier(Integer.MAX_VALUE));
    }

    // ── payload 拒收分支 ─────────────────────────────────────────────────────

    @Test
    void null_payload_is_rejected() {
        assertFalse(RatQiTierHandler.handleSync(null, 10), "null payload 必须拒收");
    }

    @Test
    void negative_size_is_rejected() {
        assertFalse(RatQiTierHandler.handleSync(sync("[]"), -1), "负字节数必须拒收");
    }

    @Test
    void oversize_payload_is_rejected() {
        assertFalse(
            RatQiTierHandler.handleSync(sync("[]"), ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1),
            "超过 MAX_PAYLOAD_BYTES 的 payload 必须拒收"
        );
    }

    @Test
    void malformed_json_is_rejected() {
        assertFalse(feed("{not json"), "非法 JSON 必须拒收而非抛异常");
        assertFalse(feed("[1,2,3]"), "顶层非对象必须拒收");
    }

    @Test
    void wrong_version_is_rejected() {
        assertFalse(feed("{\"v\":2,\"type\":\"rat_qi_tier\",\"entries\":[]}"),
            "版本号不为 1 必须拒收（防未来 wire 变更被旧 client 误读）");
        assertFalse(feed("{\"type\":\"rat_qi_tier\",\"entries\":[]}"), "缺 v 字段必须拒收");
    }

    @Test
    void wrong_type_is_rejected() {
        assertFalse(feed("{\"v\":1,\"type\":\"spider_disguise_enter\",\"entries\":[]}"),
            "type 不匹配必须拒收——否则串 channel 的 payload 会污染档位表");
        assertFalse(feed("{\"v\":1,\"entries\":[]}"), "缺 type 字段必须拒收");
    }

    @Test
    void rejected_payload_leaves_previous_state_intact() {
        feed(sync("[{\"id\":21,\"tier\":2}]"));
        assertFalse(feed("{not json"));
        assertEquals(2, RatQiTierHandler.tierOf(21),
            "拒收的 payload 不得清空/污染既有档位表（现态不变）");
    }

    @Test
    void missing_entries_field_is_accepted_as_empty() {
        feed(sync("[{\"id\":22,\"tier\":2}]"));
        assertTrue(feed("{\"v\":1,\"type\":\"rat_qi_tier\"}"),
            "缺 entries 字段视同空名单（合法 keepalive）");
        assertEquals(0, RatQiTierHandler.tierOf(22), "视同空名单 → 全部降回 0 档");
    }

    @Test
    void null_entries_field_is_accepted_as_empty() {
        assertTrue(feed("{\"v\":1,\"type\":\"rat_qi_tier\",\"entries\":null}"));
        assertTrue(RatQiTierHandler.snapshot().isEmpty());
    }

    @Test
    void non_array_entries_field_is_accepted_as_empty() {
        assertTrue(feed("{\"v\":1,\"type\":\"rat_qi_tier\",\"entries\":{\"id\":1}}"),
            "entries 不是数组时按空名单处理，不抛异常");
        assertTrue(RatQiTierHandler.snapshot().isEmpty());
    }

    // ── 断线清理 ─────────────────────────────────────────────────────────────

    @Test
    void disconnect_clears_all_tiers() {
        feed(sync("[{\"id\":31,\"tier\":2},{\"id\":32,\"tier\":1}]"));
        RatQiTierHandler.clearOnDisconnect();
        assertEquals(0, RatQiTierHandler.tierOf(31),
            "断线必须清表——entity id 会被下个 session 重新分配给别的实体，留着会串档");
        assertEquals(0, RatQiTierHandler.tierOf(32));
        assertTrue(RatQiTierHandler.snapshot().isEmpty());
    }

    // ── 常量与服务端契约对齐 ─────────────────────────────────────────────────

    @Test
    void wire_constants_pin() {
        assertEquals("bong", RatQiTierHandler.CHANNEL_NAMESPACE);
        assertEquals("rat_qi_tier", RatQiTierHandler.CHANNEL_PATH,
            "channel path 必须与服务端 ident!(\"bong:rat_qi_tier\") 逐字一致");
        assertEquals("rat_qi_tier", RatQiTierHandler.PAYLOAD_TYPE,
            "payload type 必须与服务端 RatQiTierS2c::sync 的 ty 字段逐字一致");
        assertEquals(2, RatQiTierHandler.TIER_MAX,
            "档位上限必须与服务端 RAT_QI_TIER_MAX 同值（贴图只有 q0/q1/q2）");
        assertEquals(0, RatQiTierHandler.TIER_DEFAULT);
    }

    @Test
    void snapshot_is_unmodifiable() {
        feed(sync("[{\"id\":41,\"tier\":1}]"));
        Map<Integer, Integer> snapshot = RatQiTierHandler.snapshot();
        try {
            snapshot.put(99, 2);
            throw new AssertionError("snapshot 应为只读，put 必须抛 UnsupportedOperationException");
        } catch (UnsupportedOperationException expected) {
            // 预期
        }
    }
}
