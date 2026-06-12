package com.bong.client.agentui;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P3 — AgentUiVfxState + AgentUiVfxPlanner 单测。
 *
 * <p>测试范围：
 * <ul>
 *   <li>AgentUiVfxState.fadeInProgress: t=0→0.0 / 半途 / t≥FADE_IN_DURATION_MS→1.0 / clamp ≤0</li>
 *   <li>AgentUiVfxState.tiandaoVignetteActive: 仅 isTiandaoRevelation=true 时活跃；过期后 false</li>
 *   <li>AgentUiVfxState.tiandaoShakeActive: 仅 isTiandaoRevelation=true 时活跃；过期后 false</li>
 *   <li>AgentUiVfxPlanner.buildCommands: null state → empty / fade-in 中 → SCREEN_TINT / 完成后无 tint</li>
 *   <li>AgentUiVfxPlanner: tiandao vignette → EDGE_VIGNETTE AGENT_UI layer</li>
 *   <li>AgentUiVfxPlanner: tiandao shake → RECT 命令（含 top/bottom bars）</li>
 *   <li>AgentUiVfxPlanner: 非 tiandao → 无 vignette / 无 shake</li>
 *   <li>AgentUiVfxPlanner: 零屏幕尺寸 → shake 命令不生成（防止 height-1 = -1）</li>
 *   <li>TIANDAO_VIGNETTE_ARGB 值正确（#661A0D40）</li>
 *   <li>AgentUiScreen.create isTiandaoRevelation=true → screen.isTiandaoRevelation()=true</li>
 *   <li>AgentUiScreen.create (default) → isTiandaoRevelation=false</li>
 *   <li>AgentUiVfxStore: setActive / getActive / clear 生命周期</li>
 * </ul>
 */
public class AgentUiVfxPlannerTest {

    // ─── AgentUiVfxState ─────────────────────────────────────────────────────

    @Test
    void fadeInProgress_atOpenTime_isZero() {
        long now = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(now, false);
        assertEquals(0.0, state.fadeInProgress(now), 1e-6,
            "打开时刻 fadeInProgress 应为 0.0（全透明）");
    }

    @Test
    void fadeInProgress_halfway_isHalf() {
        long opened = 1_000_000L;
        long half = opened + AgentUiVfxState.FADE_IN_DURATION_MS / 2;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        double progress = state.fadeInProgress(half);
        assertEquals(0.5, progress, 0.02,
            "halfway 时 fadeInProgress 应约为 0.5，实际=" + progress);
    }

    @Test
    void fadeInProgress_afterDuration_isOne() {
        long opened = 1_000_000L;
        long after = opened + AgentUiVfxState.FADE_IN_DURATION_MS + 100;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        assertEquals(1.0, state.fadeInProgress(after), 1e-6,
            "超过 FADE_IN_DURATION_MS 后 fadeInProgress 应 clamp 到 1.0");
    }

    @Test
    void fadeInProgress_beforeOpen_clampsToZero() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        // 在打开时刻之前（nowMs < openedAtMs）
        assertEquals(0.0, state.fadeInProgress(opened - 100), 1e-6,
            "打开前 fadeInProgress 应 clamp 到 0.0");
    }

    @Test
    void tiandaoVignetteActive_nonTiandao_false() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        assertFalse(state.tiandaoVignetteActive(opened),
            "非天道启示面板不应有 vignette active");
    }

    @Test
    void tiandaoVignetteActive_tiandao_duringDuration_true() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        // 刚打开时
        assertTrue(state.tiandaoVignetteActive(opened),
            "天道启示面板打开时 tiandaoVignetteActive 应为 true");
        // 在 duration 内
        assertTrue(state.tiandaoVignetteActive(opened + AgentUiVfxState.TIANDAO_VIGNETTE_DURATION_MS - 1),
            "在 TIANDAO_VIGNETTE_DURATION_MS - 1ms 时仍应 active");
    }

    @Test
    void tiandaoVignetteActive_tiandao_afterDuration_false() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        assertFalse(state.tiandaoVignetteActive(opened + AgentUiVfxState.TIANDAO_VIGNETTE_DURATION_MS),
            "超过 TIANDAO_VIGNETTE_DURATION_MS 后 tiandaoVignetteActive 应为 false");
    }

    @Test
    void tiandaoShakeActive_nonTiandao_false() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        assertFalse(state.tiandaoShakeActive(opened),
            "非天道启示面板不应有 shake active");
    }

    @Test
    void tiandaoShakeActive_tiandao_duringShakeDuration_true() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        assertTrue(state.tiandaoShakeActive(opened),
            "天道启示面板打开时 tiandaoShakeActive 应为 true");
        assertTrue(state.tiandaoShakeActive(opened + AgentUiVfxState.TIANDAO_SHAKE_DURATION_MS - 1),
            "在 TIANDAO_SHAKE_DURATION_MS - 1ms 时仍应 shake active");
    }

    @Test
    void tiandaoShakeActive_tiandao_afterShakeDuration_false() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        assertFalse(state.tiandaoShakeActive(opened + AgentUiVfxState.TIANDAO_SHAKE_DURATION_MS),
            "超过 TIANDAO_SHAKE_DURATION_MS 后 tiandaoShakeActive 应为 false");
    }

    @Test
    void vignetteArgbConstant_correctValue() {
        // #1A0D40 ARGB with alpha=0x66 (opacity 0.4 × 255 ≈ 102)
        assertEquals(0x661A0D40, AgentUiVfxState.TIANDAO_VIGNETTE_ARGB,
            "TIANDAO_VIGNETTE_ARGB 应为 0x661A0D40（#1A0D40，opacity 0.4），实际=0x"
            + Integer.toHexString(AgentUiVfxState.TIANDAO_VIGNETTE_ARGB));
    }

    @Test
    void fadeInDuration_is500ms() {
        assertEquals(500L, AgentUiVfxState.FADE_IN_DURATION_MS,
            "FADE_IN_DURATION_MS 应为 500ms（10t × 50ms/t）");
    }

    // ─── AgentUiVfxPlanner.buildCommands ─────────────────────────────────────

    @Test
    void buildCommands_nullState_returnsEmpty() {
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(null, 1_000_000L, 800, 600);
        assertTrue(cmds.isEmpty(),
            "null state 时 buildCommands 应返回空列表，实际大小=" + cmds.size());
    }

    @Test
    void buildCommands_duringFadeIn_containsScreenTint() {
        long opened = 1_000_000L;
        // 用 nowMs = opened（progress=0，alpha 最大）
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, opened, 800, 600);

        boolean hasScreenTint = cmds.stream()
            .anyMatch(c -> c.layer() == HudRenderLayer.AGENT_UI && c.kind() == HudRenderCommand.Kind.SCREEN_TINT);
        assertTrue(hasScreenTint,
            "fade-in 开始时应有 SCREEN_TINT 命令（AGENT_UI layer），cmds=" + cmds);
    }

    @Test
    void buildCommands_afterFadeIn_noScreenTint() {
        long opened = 1_000_000L;
        long after = opened + AgentUiVfxState.FADE_IN_DURATION_MS + 200;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, after, 800, 600);

        boolean hasScreenTint = cmds.stream()
            .anyMatch(c -> c.kind() == HudRenderCommand.Kind.SCREEN_TINT);
        assertFalse(hasScreenTint,
            "fade-in 完成后不应有 SCREEN_TINT 命令，cmds=" + cmds);
    }

    @Test
    void buildCommands_tiandao_duringVignette_containsEdgeVignette() {
        long opened = 1_000_000L;
        long after = opened + AgentUiVfxState.FADE_IN_DURATION_MS + 100; // fade done, vignette active
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, after, 800, 600);

        boolean hasVignette = cmds.stream()
            .anyMatch(c -> c.layer() == HudRenderLayer.AGENT_UI && c.kind() == HudRenderCommand.Kind.EDGE_VIGNETTE);
        assertTrue(hasVignette,
            "天道启示 vignette 期间应有 EDGE_VIGNETTE 命令（AGENT_UI layer），cmds=" + cmds);
    }

    @Test
    void buildCommands_nonTiandao_noEdgeVignette() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, false);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state,
            opened + AgentUiVfxState.FADE_IN_DURATION_MS + 100, 800, 600);

        boolean hasVignette = cmds.stream()
            .anyMatch(c -> c.kind() == HudRenderCommand.Kind.EDGE_VIGNETTE);
        assertFalse(hasVignette,
            "非天道启示面板不应有 EDGE_VIGNETTE 命令，cmds=" + cmds);
    }

    @Test
    void buildCommands_tiandao_duringShake_containsRectBars() {
        long opened = 1_000_000L;
        // Use timestamps where sin(phase) > 0.01 to ensure shake is visible.
        AgentUiVfxState state = new AgentUiVfxState(opened, true);

        // Try multiple timestamps to find one where shake renders (sin non-zero)
        boolean foundShake = false;
        for (int ms = 10; ms < 2000; ms += 15) {
            List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(
                state, opened + ms, 800, 600);
            long rectCount = cmds.stream()
                .filter(c -> c.kind() == HudRenderCommand.Kind.RECT && c.layer() == HudRenderLayer.AGENT_UI)
                .count();
            if (rectCount >= 2) {
                foundShake = true;
                break;
            }
        }
        assertTrue(foundShake,
            "天道启示 shake 期间应在某 tick 出现至少 2 个 RECT 命令（top/bottom bar）");
    }

    @Test
    void buildCommands_zeroScreenSize_noShakeRect() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, opened + 50, 0, 0);

        boolean hasRect = cmds.stream().anyMatch(c -> c.kind() == HudRenderCommand.Kind.RECT);
        assertFalse(hasRect,
            "屏幕尺寸为 0 时不应生成 RECT shake 命令（防 height-1=-1），cmds=" + cmds);
    }

    @Test
    void buildCommands_tiandao_afterVignetteDuration_noVignette() {
        long opened = 1_000_000L;
        long expired = opened + AgentUiVfxState.TIANDAO_VIGNETTE_DURATION_MS + 1000;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, expired, 800, 600);

        boolean hasVignette = cmds.stream()
            .anyMatch(c -> c.kind() == HudRenderCommand.Kind.EDGE_VIGNETTE);
        assertFalse(hasVignette,
            "天道启示 vignette 超时后不应有 EDGE_VIGNETTE 命令，cmds=" + cmds);
    }

    @Test
    void buildCommands_allCommandsUsedAgentUiLayer() {
        long opened = 1_000_000L;
        AgentUiVfxState state = new AgentUiVfxState(opened, true);
        List<HudRenderCommand> cmds = AgentUiVfxPlanner.buildCommands(state, opened, 800, 600);

        for (HudRenderCommand cmd : cmds) {
            assertEquals(HudRenderLayer.AGENT_UI, cmd.layer(),
                "所有 VFX 命令应使用 AGENT_UI layer，发现=" + cmd.layer());
        }
    }

    // ─── AgentUiVfxStore ──────────────────────────────────────────────────────

    @Test
    void vfxStore_setActive_getActive_lifecycle() {
        try {
            AgentUiVfxState state = new AgentUiVfxState(1_000_000L, true);
            AgentUiVfxStore.setActive(state);

            assertNotNull(AgentUiVfxStore.getActive(),
                "setActive 后 getActive 应非 null");
            assertEquals(state, AgentUiVfxStore.getActive(),
                "getActive 应返回设置的 state");
        } finally {
            AgentUiVfxStore.clear();
        }
    }

    @Test
    void vfxStore_clear_removesState() {
        try {
            AgentUiVfxStore.setActive(new AgentUiVfxState(1_000_000L, false));
            AgentUiVfxStore.clear();
            assertNull(AgentUiVfxStore.getActive(),
                "clear 后 getActive 应为 null");
        } finally {
            AgentUiVfxStore.clear();
        }
    }

    @Test
    void vfxStore_initiallyNull() {
        AgentUiVfxStore.clear(); // 确保干净状态
        assertNull(AgentUiVfxStore.getActive(),
            "初始状态 AgentUiVfxStore.getActive 应为 null");
    }

    // ─── AgentUiScreen P3 VFX 集成 ───────────────────────────────────────────

    @Test
    void agentUiScreen_create_defaultOverload_isTiandaoFalse() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-p3-001",
            "<owo-ui><components><flow-layout><label>天意</label><button id=\"dismiss\">关</button></flow-layout></components></owo-ui>",
            600, 1000L
        );
        assertFalse(screen.isTiandaoRevelation(),
            "默认 create() 重载 isTiandaoRevelation 应为 false");
        assertEquals("req-p3-001", screen.requestId());
    }

    @Test
    void agentUiScreen_create_tiandaoTrue_flagSet() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-tiandao-001",
            "<owo-ui><components><flow-layout direction=\"vertical\" gap=\"6\">"
            + "<label>天意</label>"
            + "<button id=\"dismiss\">闭目冥思</button>"
            + "</flow-layout></components></owo-ui>",
            600, 1000L,
            /* isTiandaoRevelation= */ true
        );
        assertTrue(screen.isTiandaoRevelation(),
            "create(..., isTiandaoRevelation=true) 后 isTiandaoRevelation() 应为 true");
    }

    @Test
    void agentUiScreen_create_tiandaoFalse_flagCleared() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-non-tiandao",
            "<owo-ui><components><flow-layout><label>x</label></flow-layout></components></owo-ui>",
            600, 1000L,
            /* isTiandaoRevelation= */ false
        );
        assertFalse(screen.isTiandaoRevelation(),
            "create(..., isTiandaoRevelation=false) 后 isTiandaoRevelation() 应为 false");
    }

    @Test
    void agentUiScreen_fallback_isTiandaoFalse() {
        // XML 解析失败 → fallback，isTiandaoRevelation 固定 false
        AgentUiScreen fallback = AgentUiScreen.create(
            "req-fallback-p3", "<<<not xml>>>", 600, 0L, true
        );
        assertFalse(fallback.isTiandaoRevelation(),
            "fallback 面板 isTiandaoRevelation 应强制为 false（parse 失败无 tiandao 语义）");
    }
}
