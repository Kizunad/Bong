package com.bong.client;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiVfxState;
import com.bong.client.agentui.AgentUiVfxStore;
import com.bong.client.combat.baomai.v3.BaomaiV3HudStateStore;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.CombatHudSnapshot;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudImmersionMode;
import com.bong.client.hud.HudLayoutPreferenceStore;
import com.bong.client.hud.HudRuntimeContext;
import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchProgressHudPlanner;
import net.minecraft.client.gui.screen.GameMenuScreen;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongHudTest {
    @BeforeEach
    void setUp() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
        BaomaiV3HudStateStore.clear();
        AgentUiVfxStore.clear();
        HudImmersionMode.resetForTests();
        HudLayoutPreferenceStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
        BaomaiV3HudStateStore.clear();
        AgentUiVfxStore.clear();
        HudImmersionMode.resetForTests();
        HudLayoutPreferenceStore.resetForTests();
    }



    @Test
    public void productionPathRendersBaomaiV3HudOnlyInFullHudMode() {
        BaomaiV3HudStateStore.recordBodyTranscendence(500, 10.0);
        long nowMs = System.currentTimeMillis();
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        BongHud.renderBaomaiV3HudForProduction(surface, nowMs, ScreenHudVisibility.FULL);

        assertTrue(surface.drawTexts.stream().anyMatch(call -> call.text().contains("凡躯重铸 x10")));

        RecordingHudSurface castOnlySurface = new RecordingHudSurface(320, 180);
        BongHud.renderBaomaiV3HudForProduction(castOnlySurface, nowMs, ScreenHudVisibility.CAST_BAR_ONLY);

        assertTrue(castOnlySurface.shadowTexts.isEmpty());
        assertTrue(castOnlySurface.drawTexts.isEmpty());
    }

    @Test
    public void agentUiScreenRunsStoreThroughProductionRenderChain() {
        long openedAtMs = 1_000L;
        long nowMillis = openedAtMs + 250L;
        AgentUiScreen screen = AgentUiScreen.create(
            "req-screen-gate",
            "<owo-ui><components><flow-layout><label>天意</label></flow-layout></components></owo-ui>",
            200,
            1_000L,
            true
        );
        AgentUiVfxStore.setActive(new AgentUiVfxState(openedAtMs, true));
        List<List<HudRenderCommand>> renderedFrames = new ArrayList<>();
        List<ScreenHudVisibility> renderedVisibilities = new ArrayList<>();
        int[] frameBuildCount = {0};

        BongHud.render(
            screen,
            nowMillis,
            () -> {
                frameBuildCount[0]++;
                return emptyHudFrameInput(320, 180);
            },
            (commands, visibility) -> {
                renderedFrames.add(commands);
                renderedVisibilities.add(visibility);
            }
        );

        assertEquals(1, frameBuildCount[0], "AgentUiScreen 不得在 orchestrator 构建前提前返回");
        assertEquals(List.of(ScreenHudVisibility.AGENT_UI_ONLY), renderedVisibilities);
        assertEquals(1, renderedFrames.size());
        List<HudRenderCommand> commands = renderedFrames.get(0);
        assertFalse(commands.isEmpty(), "真实 Store→Orchestrator 链应把 Agent UI VFX 送入 renderer");
        assertTrue(
            commands.stream().allMatch(command -> command.layer() == HudRenderLayer.AGENT_UI),
            "expected only AGENT_UI commands because AgentUiScreen isolates its VFX, actual commands=" + commands
        );
        assertTrue(
            commands.stream().anyMatch(command -> command.kind() == HudRenderCommand.Kind.SCREEN_TINT),
            "expected SCREEN_TINT because Agent UI fade-in must reach the renderer, actual commands=" + commands
        );
        assertTrue(
            commands.stream().anyMatch(command -> command.kind() == HudRenderCommand.Kind.EDGE_VIGNETTE),
            "expected EDGE_VIGNETTE because Agent UI revelation must reach the renderer, actual commands=" + commands
        );
        assertEquals(
            2L,
            commands.stream().filter(command -> command.kind() == HudRenderCommand.Kind.RECT).count(),
            "天道 shake 的上下两条 RECT 必须抵达最终 renderer"
        );
    }

    @Test
    public void agentUiScreenWithEmptyStoreStillRendersNoForeignHudLayers() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-empty-store",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            200,
            1_000L
        );
        List<List<HudRenderCommand>> renderedFrames = new ArrayList<>();

        BongHud.render(
            screen,
            1_000L,
            () -> emptyHudFrameInput(320, 180),
            (commands, visibility) -> renderedFrames.add(commands)
        );

        assertEquals(1, renderedFrames.size(), "AGENT_UI_ONLY 仍应执行最终 renderer，而非提前返回");
        assertTrue(renderedFrames.get(0).isEmpty(), "Store 为空时不得泄露 baseline 或其他 HUD layer");
    }

    @Test
    public void hiddenScreenSkipsFrameCaptureAndRenderer() {
        int[] frameBuildCount = {0};
        int[] renderCount = {0};

        BongHud.render(
            new GameMenuScreen(false),
            1_000L,
            () -> {
                frameBuildCount[0]++;
                return emptyHudFrameInput(320, 180);
            },
            (commands, visibility) -> renderCount[0]++
        );

        assertEquals(0, frameBuildCount[0], "HIDDEN 屏幕不得采样 HUD frame");
        assertEquals(0, renderCount[0], "HIDDEN 屏幕不得触发最终 renderer");
    }

    @Test
    public void productionScreenFiltersNeverLeakSearchFlashIntoOtherInterfaces() {
        List<HudRenderCommand> commands = new ArrayList<>(
            SearchProgressHudPlanner.buildCommands(SearchHudState.completed("残棺"), 320, 180)
        );
        commands.add(HudRenderCommand.text(HudRenderLayer.OVERWEIGHT, "baseline", 0, 0, 0xFFFFFF));
        commands.add(HudRenderCommand.text(HudRenderLayer.CAST_BAR, "cast", 0, 0, 0xFFFFFF));
        commands.add(HudRenderCommand.screenTint(HudRenderLayer.AGENT_UI, 0xCC000000));

        List<HudRenderCommand> full = BongHud.filterCommandsForVisibility(
            commands,
            ScreenHudVisibility.FULL
        );
        List<HudRenderCommand> inventory = BongHud.filterCommandsForVisibility(
            commands,
            ScreenHudVisibility.INVENTORY_DIMMED
        );
        List<HudRenderCommand> castOnly = BongHud.filterCommandsForVisibility(
            commands,
            ScreenHudVisibility.CAST_BAR_ONLY
        );
        List<HudRenderCommand> agentUiOnly = BongHud.filterCommandsForVisibility(
            commands,
            ScreenHudVisibility.AGENT_UI_ONLY
        );
        List<HudRenderCommand> hidden = BongHud.filterCommandsForVisibility(
            commands,
            ScreenHudVisibility.HIDDEN
        );

        assertSame(
            commands,
            full,
            "FULL 可见模式必须直接复用原命令列表，避免每帧无条件复制"
        );
        assertTrue(
            full.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "FULL 必须保留 SEARCH_PROGRESS，因为主 HUD 应显示尚未过期的搜刮 flash，实际命令=" + full
        );
        assertTrue(
            inventory.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "INVENTORY_DIMMED 不得泄漏 SEARCH_PROGRESS，因为库存界面只保留降噪层，实际命令=" + inventory
        );
        assertTrue(
            castOnly.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "CAST_BAR_ONLY 不得泄漏 SEARCH_PROGRESS，因为该模式只保留施法条，实际命令=" + castOnly
        );
        assertTrue(
            agentUiOnly.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "AGENT_UI_ONLY 不得泄漏 SEARCH_PROGRESS，因为天道界面隔离其它 HUD layer，实际命令=" + agentUiOnly
        );
        assertTrue(hidden.isEmpty(), "HIDDEN 必须过滤全部命令，实际命令=" + hidden);
        assertTrue(
            inventory.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.OVERWEIGHT),
            "INVENTORY_DIMMED 应保留 OVERWEIGHT，因为库存界面仍显示超载提示，实际命令=" + inventory
        );
        assertTrue(
            castOnly.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.CAST_BAR),
            "CAST_BAR_ONLY 应保留 CAST_BAR，因为施法界面必须显示施法进度，实际命令=" + castOnly
        );
        assertTrue(
            agentUiOnly.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.AGENT_UI),
            "AGENT_UI_ONLY 应保留 AGENT_UI，因为天道专用 VFX 必须可见，实际命令=" + agentUiOnly
        );
    }

    @Test
    public void commandVisibilityFiltersEveryPolicyWithoutLayerLeakage() {
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.text(HudRenderLayer.OVERWEIGHT, "baseline", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.QUICK_BAR, "quick", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.CAST_BAR, "cast", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.EVENT_STREAM, "event", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, "extract", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.ZONE_TRANSITION, "zone", 0, 0, 0xFFFFFF),
            HudRenderCommand.screenTint(HudRenderLayer.AGENT_UI, 0xCC000000),
            HudRenderCommand.edgeVignette(HudRenderLayer.AGENT_UI, 0x661A0D40),
            HudRenderCommand.rect(HudRenderLayer.AGENT_UI, 0, 0, 320, 1, 0x22000000)
        );

        assertSame(
            commands,
            BongHud.filterCommandsForVisibility(commands, ScreenHudVisibility.FULL),
            "FULL 必须保留原命令列表"
        );
        assertEquals(
            List.of(HudRenderLayer.CAST_BAR),
            layers(BongHud.filterCommandsForVisibility(commands, ScreenHudVisibility.CAST_BAR_ONLY))
        );
        assertEquals(
            List.of(
                HudRenderLayer.OVERWEIGHT,
                HudRenderLayer.QUICK_BAR,
                HudRenderLayer.CAST_BAR,
                HudRenderLayer.EVENT_STREAM,
                HudRenderLayer.TSY_EXTRACT
            ),
            layers(BongHud.filterCommandsForVisibility(commands, ScreenHudVisibility.INVENTORY_DIMMED))
        );
        assertEquals(
            List.of(HudRenderLayer.AGENT_UI, HudRenderLayer.AGENT_UI, HudRenderLayer.AGENT_UI),
            layers(BongHud.filterCommandsForVisibility(commands, ScreenHudVisibility.AGENT_UI_ONLY)),
            "专用策略必须保留所有 Agent UI VFX kind，同时剔除其他 HUD layer"
        );
        assertTrue(
            BongHud.filterCommandsForVisibility(commands, ScreenHudVisibility.HIDDEN).isEmpty(),
            "HIDDEN 的纯过滤契约必须返回空命令"
        );
    }

    @Test
    public void commandVisibilityHandlesEmptyAndRejectsNullInputs() {
        for (ScreenHudVisibility visibility : ScreenHudVisibility.values()) {
            assertTrue(
                BongHud.filterCommandsForVisibility(List.of(), visibility).isEmpty(),
                "空命令列表在 " + visibility + " 策略下都应保持为空"
            );
        }
        assertThrows(
            NullPointerException.class,
            () -> BongHud.filterCommandsForVisibility(null, ScreenHudVisibility.FULL)
        );
        assertThrows(
            NullPointerException.class,
            () -> BongHud.filterCommandsForVisibility(List.of(), null)
        );
    }

    private static List<HudRenderLayer> layers(List<HudRenderCommand> commands) {
        return commands.stream().map(HudRenderCommand::layer).toList();
    }

    private static BongHud.HudFrameInput emptyHudFrameInput(int screenWidth, int screenHeight) {
        return new BongHud.HudFrameInput(
            BongHudStateSnapshot.empty(),
            CombatHudSnapshot.empty(),
            text -> text == null ? 0 : text.length() * 6,
            220,
            screenWidth,
            screenHeight,
            null,
            HudRuntimeContext.empty(),
            List::of,
            List::of
        );
    }

    private static final class RecordingHudSurface implements BongHud.HudSurface {
        private final int width;
        private final int height;
        private final List<ShadowTextCall> shadowTexts = new ArrayList<>();
        private final List<FillRectCall> fillRects = new ArrayList<>();
        private final List<DrawTextCall> drawTexts = new ArrayList<>();

        private RecordingHudSurface(int width, int height) {
            this.width = width;
            this.height = height;
        }

        @Override
        public int windowWidth() {
            return width;
        }

        @Override
        public int windowHeight() {
            return height;
        }

        @Override
        public int measureText(String text) {
            return text.length() * 6;
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int color) {
            fillRects.add(new FillRectCall(x1, y1, x2, y2, color));
        }

        @Override
        public void drawTextWithShadow(String text, int x, int y, int color) {
            shadowTexts.add(new ShadowTextCall(text, x, y, color));
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            drawTexts.add(new DrawTextCall(text, x, y, color, shadow));
        }
    }

    private record ShadowTextCall(String text, int x, int y, int color) {
    }

    private record FillRectCall(int x1, int y1, int x2, int y2, int color) {
    }

    private record DrawTextCall(String text, int x, int y, int color, boolean shadow) {
    }
}
