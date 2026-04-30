package com.bong.client;

import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.SpellVolumeStore;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.UnlockedStylesStore;
import com.bong.client.hud.BongHudOrchestrator;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.BotanyProjection;
import com.bong.client.hud.CombatHudSnapshot;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.ScreenHudVisibility;
import com.bong.client.inventory.component.GridSlotComponent;
import net.minecraft.client.render.Camera;
import net.minecraft.util.math.Vec3d;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.visual.EdgeDecalRenderer;
import com.bong.client.visual.InkWashVignetteRenderer;
import com.bong.client.visual.OverlayQuadRenderer;
import com.bong.client.visual.realm_vision.EdgeIndicatorCmd;
import com.bong.client.visual.realm_vision.PerceptionEdgeProjector;
import com.bong.client.visual.realm_vision.PerceptionEdgeRenderer;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

public class BongHud {
    private static final int HUD_TEXT_MAX_WIDTH = 220;
    static final String BASELINE_STATUS_TEXT = BongHudOrchestrator.BASELINE_LABEL;
    static final int BASELINE_TEXT_COLOR = 0xFFFFFF;
    private static final int BASELINE_X = 10;
    private static final int BASELINE_Y = 10;
    private static final int TOAST_BACKGROUND_COLOR = 0x88000000;
    private static final int TOAST_HORIZONTAL_PADDING = 4;
    private static final int TOAST_VERTICAL_PADDING = 4;

    public static void render(DrawContext context, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        long nowMillis = System.currentTimeMillis();

        // Tick cast-state + defense-window expiries so they self-clear each frame.
        com.bong.client.visual.realm_vision.RealmVisionStateStore.tick();
        CastStateStore.tick(nowMillis);
        DefenseWindowStore.tick(nowMillis);
        com.bong.client.tsy.ExtractStateStore.tick(nowMillis);
        // Open death/terminate screens when the server activates them.
        com.bong.client.combat.screen.CombatScreenOpener.tick();

        Screen currentScreen = client.currentScreen;
        ScreenHudVisibility visibility = ScreenHudVisibility.forScreen(currentScreen);
        if (visibility == ScreenHudVisibility.HIDDEN) {
            return;
        }

        CombatHudSnapshot combatSnapshot = captureCombatSnapshot(client);

        BotanyProjection.Anchor botanyAnchor = computeBotanyAnchor(client);

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateStore.snapshot(),
            combatSnapshot,
            nowMillis,
            client.textRenderer::getWidth,
            HUD_TEXT_MAX_WIDTH,
            client.getWindow().getScaledWidth(),
            client.getWindow().getScaledHeight(),
            botanyAnchor
        );
        List<EdgeIndicatorCmd> spiritualSenseIndicators = computeSpiritualSenseIndicators(client);
        if (!spiritualSenseIndicators.isEmpty()) {
            commands = new ArrayList<>(commands);
            PerceptionEdgeRenderer.append(commands, spiritualSenseIndicators);
        }

        if (visibility == ScreenHudVisibility.CAST_BAR_ONLY) {
            commands = filterCastBarOnly(commands);
        } else if (visibility == ScreenHudVisibility.INVENTORY_DIMMED) {
            commands = filterInventoryDimmed(commands);
        }

        for (HudRenderCommand command : commands) {
            if (command.isText()) {
                context.drawTextWithShadow(client.textRenderer, command.text(), command.x(), command.y(), command.color());
                continue;
            }
            if (command.isRect()) {
                context.fill(command.x(), command.y(), command.x() + command.width(), command.y() + command.height(), command.color());
                continue;
            }
            if (command.isTexturedRect()) {
                Identifier tex = parseIdentifier(command.texturePath());
                if (tex != null) {
                    context.drawTexture(
                        tex,
                        command.x(), command.y(),
                        0.0f, 0.0f,
                        command.width(), command.height(),
                        command.width(), command.height()
                    );
                }
                continue;
            }
            if (command.isItemTexture()) {
                drawItemTexture(context, command.text(), command.x(), command.y(), command.width());
                continue;
            }
            if (command.isToast()) {
                BongToast.render(
                    context,
                    client.textRenderer,
                    client.getWindow().getScaledWidth(),
                    client.getWindow().getScaledHeight(),
                    command
                );
                continue;
            }
            if (command.isEdgeIndicator()) {
                int size = Math.max(4, (int) Math.round(4.0 + command.intensity() * 6.0));
                context.fill(
                    command.x() - size,
                    command.y() - size,
                    command.x() + size,
                    command.y() + size,
                    command.color()
                );
            }
        }

        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();
        for (HudRenderCommand command : commands) {
            if (command.isScreenTint()) {
                OverlayQuadRenderer.render(context, scaledWidth, scaledHeight, command.color());
            } else if (command.isEdgeVignette()) {
                EdgeDecalRenderer.render(context, scaledWidth, scaledHeight, command.color());
            } else if (command.isEdgeInkWash()) {
                InkWashVignetteRenderer.render(context, scaledWidth, scaledHeight, command.color());
            }
        }
    }

    private static List<EdgeIndicatorCmd> computeSpiritualSenseIndicators(MinecraftClient client) {
        PerceptionEdgeState state = PerceptionEdgeStateStore.snapshot();
        if (state.isEmpty() || client.gameRenderer == null) {
            return List.of();
        }
        Camera camera = client.gameRenderer.getCamera();
        if (camera == null) {
            return List.of();
        }
        Vec3d camPos = camera.getPos();
        double fov = client.options.getFov().getValue().doubleValue();
        int scaledWidth = client.getWindow().getScaledWidth();
        int scaledHeight = client.getWindow().getScaledHeight();
        List<EdgeIndicatorCmd> indicators = new ArrayList<>();
        for (PerceptionEdgeState.SenseEntry entry : state.entries()) {
            indicators.add(PerceptionEdgeProjector.project(
                entry.x(), entry.y(), entry.z(),
                camPos.x, camPos.y, camPos.z,
                camera.getYaw(), camera.getPitch(),
                fov,
                scaledWidth,
                scaledHeight,
                entry.kind(),
                entry.intensity()
            ));
        }
        return indicators;
    }

    private static Identifier parseIdentifier(String path) {
        if (path == null || path.isBlank()) {
            return null;
        }
        try {
            return new Identifier(path);
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static BotanyProjection.Anchor computeBotanyAnchor(MinecraftClient client) {
        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.hasTargetPos() || client.gameRenderer == null) {
            return null;
        }
        Camera camera = client.gameRenderer.getCamera();
        if (camera == null) {
            return null;
        }
        Vec3d camPos = camera.getPos();
        double[] pos = session.targetPos();
        double fov = client.options.getFov().getValue().doubleValue();
        return BotanyProjection.project(
            pos[0], pos[1], pos[2],
            camPos.x, camPos.y, camPos.z,
            camera.getYaw(), camera.getPitch(),
            fov,
            client.getWindow().getScaledWidth(),
            client.getWindow().getScaledHeight()
        );
    }

    private static CombatHudSnapshot captureCombatSnapshot(MinecraftClient client) {
        int selectedSlot = -1;
        PlayerEntity player = client.player;
        if (player != null) {
            selectedSlot = player.getInventory().selectedSlot;
        }
        return CombatHudSnapshot.create(
            CombatHudStateStore.snapshot(),
            PhysicalBodyStore.snapshot(),
            QuickUseSlotStore.snapshot(),
            SkillBarStore.snapshot(),
            selectedSlot,
            CastStateStore.snapshot(),
            UnifiedEventStore.stream(),
            SpellVolumeStore.snapshot(),
            DefenseWindowStore.snapshot(),
            UnlockedStylesStore.snapshot()
        );
    }

    private static List<HudRenderCommand> filterCastBarOnly(List<HudRenderCommand> commands) {
        return commands.stream()
            .filter(cmd -> cmd.layer() == com.bong.client.hud.HudRenderLayer.CAST_BAR)
            .toList();
    }

    private static List<HudRenderCommand> filterInventoryDimmed(List<HudRenderCommand> commands) {
        return commands.stream()
            .filter(cmd -> {
                com.bong.client.hud.HudRenderLayer layer = cmd.layer();
                // Keep quick-bar + event-stream + cast-bar; dim/hide everything else.
                return layer == com.bong.client.hud.HudRenderLayer.QUICK_BAR
                    || layer == com.bong.client.hud.HudRenderLayer.CAST_BAR
                    || layer == com.bong.client.hud.HudRenderLayer.EVENT_STREAM
                    || layer == com.bong.client.hud.HudRenderLayer.TSY_EXTRACT
                    || layer == com.bong.client.hud.HudRenderLayer.BASELINE;
            })
            .toList();
    }

    static HudSnapshot snapshot(long nowMs) {
        return new HudSnapshot(
            BASELINE_STATUS_TEXT,
            NarrationState.getCurrentToast(nowMs),
            ZoneState.getCurrentZone(),
            EventAlertState.getCurrentBanner(nowMs),
            nowMs
        );
    }

    static void renderSurface(HudSurface surface, HudSnapshot snapshot) {
        Objects.requireNonNull(surface, "surface");
        Objects.requireNonNull(snapshot, "snapshot");

        surface.drawTextWithShadow(snapshot.baselineText(), BASELINE_X, BASELINE_Y, BASELINE_TEXT_COLOR);
        BongZoneHud.render(surface, snapshot.zone(), snapshot.nowMs());
        BongEventAlertOverlay.render(surface, snapshot.eventAlert());
        renderToast(surface, snapshot.toast());
        com.bong.client.lingtian.LingtianSessionHud.render(
            surface,
            com.bong.client.lingtian.state.LingtianSessionStore.snapshot()
        );
    }

    /**
     * Draw a 128×128 source PNG (`bong-client:textures/gui/items/{itemId}.png`)
     * scaled into a {@code size×size} box at {@code (dx, dy)}. Mirrors the
     * approach used in {@code GridSlotComponent.drawItemTexture}.
     */
    private static void drawItemTexture(DrawContext context, String itemId, int dx, int dy, int size) {
        if (itemId == null || itemId.isEmpty() || size <= 0) return;
        Identifier tex = GridSlotComponent.textureIdForItemId(itemId);

        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.enableDepthTest();

        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(dx, dy, 100);
        float scale = (float) size / 128.0f;
        matrices.scale(scale, scale, 1.0f);

        context.drawTexture(tex, 0, 0, 128, 128, 0, 0, 128, 128, 128, 128);

        matrices.pop();
        RenderSystem.disableBlend();
    }

    private static void renderToast(HudSurface surface, NarrationState.ToastState toast) {
        if (toast == null || toast.text().isBlank()) {
            return;
        }

        int width = surface.measureText(toast.text());
        int x = (surface.windowWidth() - width) / 2;
        int y = surface.windowHeight() / 4;
        surface.fill(
            x - TOAST_HORIZONTAL_PADDING,
            y - TOAST_VERTICAL_PADDING,
            x + width + TOAST_HORIZONTAL_PADDING,
            y + 12,
            TOAST_BACKGROUND_COLOR
        );
        surface.drawText(toast.text(), x, y, toast.color(), true);
    }

    public interface HudSurface {
        int windowWidth();

        int windowHeight();

        int measureText(String text);

        void fill(int x1, int y1, int x2, int y2, int color);

        void drawTextWithShadow(String text, int x, int y, int color);

        void drawText(String text, int x, int y, int color, boolean shadow);
    }

    record HudSnapshot(
        String baselineText,
        NarrationState.ToastState toast,
        ZoneState.ZoneHudState zone,
        EventAlertState.BannerState eventAlert,
        long nowMs
    ) {
        HudSnapshot {
            Objects.requireNonNull(baselineText, "baselineText");
        }
    }
}
