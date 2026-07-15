package com.bong.client;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiVfxState;
import com.bong.client.agentui.AgentUiVfxStore;
import com.bong.client.combat.baomai.v3.BaomaiV3HudStateStore;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudImmersionMode;
import com.bong.client.hud.HudLayoutPreferenceStore;
import com.bong.client.hud.ScreenHudVisibility;
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
    public void emptyStateStillRendersBaselineWithoutToast() {
        BongHud.HudSnapshot snapshot = BongHud.snapshot(1_000L);
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        assertEquals("Bong Client Connected", snapshot.baselineText());
        assertNull(snapshot.toast());
        assertDoesNotThrow(() -> BongHud.renderSurface(surface, snapshot));

        assertEquals(1, surface.shadowTexts.size());
        assertEquals("Bong Client Connected", surface.shadowTexts.get(0).text());
        assertEquals(10, surface.shadowTexts.get(0).x());
        assertEquals(10, surface.shadowTexts.get(0).y());
        assertTrue(surface.fillRects.isEmpty());
        assertTrue(surface.drawTexts.isEmpty());
    }

    @Test
    public void toastStateRendersCenteredOverlay() {
        NarrationState.recordNarration(
                new BongServerPayload.Narration("broadcast", "雷劫将至，速避高处。", "system_warning", null),
                1_000L,
                ignored -> {
                }
        );

        BongHud.HudSnapshot snapshot = BongHud.snapshot(2_000L);
        RecordingHudSurface surface = new RecordingHudSurface(200, 100);

        assertDoesNotThrow(() -> BongHud.renderSurface(surface, snapshot));

        assertEquals(1, surface.shadowTexts.size());
        assertEquals(1, surface.fillRects.size());
        assertEquals(1, surface.drawTexts.size());
        assertEquals(snapshot.toast().text(), surface.drawTexts.get(0).text());
        assertEquals(0xFF5555, surface.drawTexts.get(0).color());
        assertTrue(surface.drawTexts.get(0).shadow());

        int expectedWidth = surface.measureText(snapshot.toast().text());
        assertEquals((200 - expectedWidth) / 2, surface.drawTexts.get(0).x());
        assertEquals(25, surface.drawTexts.get(0).y());
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
                return BongHud.HudFrameInput.empty(320, 180);
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
        assertTrue(commands.stream().allMatch(command -> command.layer() == HudRenderLayer.AGENT_UI));
        assertTrue(commands.stream().anyMatch(command -> command.kind() == HudRenderCommand.Kind.SCREEN_TINT));
        assertTrue(commands.stream().anyMatch(command -> command.kind() == HudRenderCommand.Kind.EDGE_VIGNETTE));
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
            () -> BongHud.HudFrameInput.empty(320, 180),
            (commands, visibility) -> renderedFrames.add(commands)
        );

        assertEquals(1, renderedFrames.size(), "AGENT_UI_ONLY 仍应执行最终 renderer，而非提前返回");
        assertTrue(renderedFrames.get(0).isEmpty(), "Store 为空时不得泄露 baseline 或其他 HUD layer");
    }

    @Test
    public void commandVisibilityFiltersEveryPolicyWithoutLayerLeakage() {
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.text(HudRenderLayer.BASELINE, "baseline", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.QUICK_BAR, "quick", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.CAST_BAR, "cast", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.EVENT_STREAM, "event", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, "extract", 0, 0, 0xFFFFFF),
            HudRenderCommand.text(HudRenderLayer.ZONE, "zone", 0, 0, 0xFFFFFF),
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
                HudRenderLayer.BASELINE,
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
