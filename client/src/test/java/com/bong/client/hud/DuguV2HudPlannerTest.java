package com.bong.client.hud;

import com.bong.client.combat.handler.DuguV2ServerDataHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class DuguV2HudPlannerTest {

    private DuguV2ServerDataHandler handler;

    @BeforeEach
    void setUp() {
        handler = new DuguV2ServerDataHandler();
        DuguV2HudStateStore.resetForTests();
    }

    @AfterEach
    void resetStore() {
        DuguV2HudStateStore.resetForTests();
    }

    // ─── helper ──────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }

    @Test
    void activeStateShowsTaintRevealSelfCureAndShroud() {
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            true,
            0.7f,
            "蛊毒入髓",
            0.45f,
            62.5f,
            true,
            true,
            10_000L
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_TAINT_WARNING && cmd.isEdgeVignette()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_TAINT_INDICATOR && cmd.isText()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_REVEAL_RISK && cmd.isRect()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_SELF_CURE_PROGRESS && cmd.isText()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_SHROUD && cmd.isScreenTint()));
    }

    @Test
    void emptyStateDoesNotEmitDuguHud() {
        assertTrue(DuguV2HudPlanner.buildCommands(DuguV2HudStateStore.State.NONE, 960, 540, 1_000L).isEmpty());
    }

    // ─── P5 新增：self_revealed suffix 测试 ────────────────────────

    @Test
    void selfRevealedTrueAddsRevealedSuffixInSelfCureText() {
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            false, 0f, "", 0f,
            50.0f,   // selfCurePercent > 0 触发渲染
            true,    // selfRevealed = true → DuguV2HudPlanner suffix=" 已露"
            false, 0L
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(
            commands.stream()
                .filter(c -> c.layer() == HudRenderLayer.DUGU_SELF_CURE_PROGRESS && c.isText())
                .anyMatch(c -> c.text() != null && c.text().contains("已露")),
            "selfRevealed=true 应在 DUGU_SELF_CURE_PROGRESS text 中出现 '已露' suffix；"
                + "实际 commands=" + commands
        );
    }

    @Test
    void selfRevealedFalseNoRevealedSuffix() {
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            false, 0f, "", 0f,
            30.0f,
            false,   // selfRevealed = false → 无 suffix
            false, 0L
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertFalse(
            commands.stream()
                .filter(c -> c.layer() == HudRenderLayer.DUGU_SELF_CURE_PROGRESS && c.isText())
                .anyMatch(c -> c.text() != null && c.text().contains("已露")),
            "selfRevealed=false 时 DUGU_SELF_CURE_PROGRESS text 不应含 '已露'；实际 commands=" + commands
        );
    }

    // ─── P5 新增：shroud 持续时间测试 ────────────────────────────

    @Test
    void shroudActiveTrueAndNotExpiredShowsTint() {
        long nowMillis = 1_000L;
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            false, 0f, "", 0f, 0f, false,
            true,
            nowMillis + 5_000L  // shroudUntilMs > nowMillis → 未过期
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, nowMillis);

        assertTrue(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_SHROUD && c.isScreenTint()),
            "shroudActive=true 且 shroudUntilMs > nowMillis 应产生 DUGU_SHROUD screenTint；"
                + "实际 commands=" + commands
        );
    }

    @Test
    void shroudExpiredDoesNotShowTint() {
        long nowMillis = 10_000L;
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            false, 0f, "", 0f, 0f, false,
            true,
            nowMillis - 100L  // shroudUntilMs < nowMillis → 已过期
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, nowMillis);

        assertFalse(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_SHROUD),
            "shroud 已过期（shroudUntilMs < nowMillis）不应产生 DUGU_SHROUD layer；实际 commands=" + commands
        );
    }

    // ─── P5 新增：各维度独立不互覆（per-dimension merge 验证）────

    @Test
    void shroudPayloadDoesNotClearSelfRevealedInStore() {
        // 通过 DuguV2HudStateStore 直接验证 replace 语义（不经 handler）
        // 先设置 selfRevealed=true + shroudActive=false
        DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
            false, 0f, "", 0f, 40f, true, false, 0L
        ));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(), "初始 selfRevealed 应为 true");

        // 外部 replace shroud 状态（模拟 per-dimension merge）
        DuguV2HudStateStore.State cur = DuguV2HudStateStore.snapshot();
        DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
            cur.tainted(), cur.taintIntensity(), cur.taintHint(), cur.revealRisk(),
            cur.selfCurePercent(), cur.selfRevealed(),  // selfRevealed 保持
            true, System.currentTimeMillis() + 5000L   // shroud 激活
        ));

        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "per-dimension merge 后 selfRevealed 不应丢失；实际=" + DuguV2HudStateStore.snapshot().selfRevealed());
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "shroudActive 应为 true；实际=" + DuguV2HudStateStore.snapshot().shroudActive());
    }

    // ─── P5 major fix 1：revealRisk handler→store→planner 全链 ────

    @Test
    void skillCastHandlerToStoreToPlannerRendersRevealRisk() {
        // red-when-reverted：把 handler 中写 revealRisk 那行删掉 → store.revealRisk=0 → planner 无命令 → 测试红
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"reveal_probability\":0.7,\"tick\":42}"
        ));

        // 验证 store 确实被写入（handler→store）
        assertEquals(0.7f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "handler.handle(skill_cast reveal_probability=0.7) → store.revealRisk 必须为 0.7；"
                + "实际=" + DuguV2HudStateStore.snapshot().revealRisk());

        // 验证 planner 产出 DUGU_REVEAL_RISK 渲染命令（store→planner）
        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(
            DuguV2HudStateStore.snapshot(), 960, 540, 1000L
        );
        assertTrue(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_REVEAL_RISK && c.isRect()),
            "handler→store→planner 全链：store.revealRisk=0.7 应产 DUGU_REVEAL_RISK rect 命令；"
                + "实际 commands=" + commands
        );
        assertTrue(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_REVEAL_RISK && c.isText()),
            "handler→store→planner 全链：store.revealRisk=0.7 应产 DUGU_REVEAL_RISK text 命令（暴露百分比）；"
                + "实际 commands=" + commands
        );
    }

    @Test
    void skillCastRevealRiskZeroPlannerProducesNoRevealCommand() {
        // reveal_probability=0 → planner 不渲染 DUGU_REVEAL_RISK
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"reverse\",\"reveal_probability\":0.0,\"tick\":5}"
        ));
        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(
            DuguV2HudStateStore.snapshot(), 960, 540, 1000L
        );
        assertFalse(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_REVEAL_RISK),
            "revealRisk=0 → planner 不应产 DUGU_REVEAL_RISK 命令；实际 commands=" + commands
        );
    }

    @Test
    void skillCastRevealRiskMergeDoesNotClearShroudInPlanner() {
        // 先 shroud，再 skill_cast，planner 应同时渲染 DUGU_SHROUD + DUGU_REVEAL_RISK
        long nowMs = 1000L;
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.8,\"expires_at_tick\":500,\"tick\":100}"
        ));
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"reveal_probability\":0.5,\"tick\":200}"
        ));
        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(
            DuguV2HudStateStore.snapshot(), 960, 540, nowMs
        );
        assertTrue(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_REVEAL_RISK),
            "per-dimension merge 后 planner 应有 DUGU_REVEAL_RISK；实际 commands=" + commands
        );
        assertTrue(
            commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.DUGU_SHROUD),
            "per-dimension merge 后 planner 应有 DUGU_SHROUD（shroud 维度未被清零）；实际 commands=" + commands
        );
    }

    @Test
    void duguV2AnimationResourcesArePackaged() {
        ClassLoader loader = Thread.currentThread().getContextClassLoader();

        assertNotNull(loader.getResource("assets/bong/player_animation/dugu_needle_throw.json"));
        assertNotNull(loader.getResource("assets/bong/player_animation/dugu_self_cure_pose.json"));
        assertNotNull(loader.getResource("assets/bong/player_animation/dugu_shroud_activate.json"));
        assertNotNull(loader.getResource("assets/bong/player_animation/dugu_pointing_curse.json"));
    }
}
