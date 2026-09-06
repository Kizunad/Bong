package com.bong.client;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.StatusEffectHudPlanner;
import com.bong.client.hud.svg.SvgHudBackend;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class BongHudRenderOrderTest {
    private static final int ICON_COLOR = 0xFF55AA77;

    @BeforeEach
    @AfterEach
    void clearStatusEffects() {
        StatusEffectStore.clear();
    }

    @Test
    void statusIconRemainsAboveSvgBackgroundAndBelowItsForeground() {
        StatusEffectStore.replace(List.of(new StatusEffectStore.Effect(
            "bleeding", "出血", StatusEffectStore.Kind.DOT,
            2, 30_000L, 0xFFE04040, "", 0
        )));
        List<HudRenderCommand> commands = StatusEffectHudPlanner.buildCommands(100, 80);
        BufferedCanvas canvas = new BufferedCanvas(100, 80);

        render(commands, canvas);

        HudRenderCommand icon = commands.stream().filter(HudRenderCommand::isTexturedRect).findFirst().orElseThrow();
        assertEquals(ICON_COLOR, canvas.pixel(icon.x() + icon.width() / 2, icon.y() + icon.height() / 2),
            "延迟提交的 SVG 背景不得盖住已立即绘制的状态 PNG 图标");
        HudRenderCommand remainingBar = commands.stream()
            .filter(command -> command.isRect() && command.height() == 1).findFirst().orElseThrow();
        assertEquals(remainingBar.color(), canvas.pixel(remainingBar.x(), remainingBar.y()),
            "图标之后的 SVG 剩余时间条仍须覆盖图标底部");
        HudRenderCommand stacks = commands.stream().filter(HudRenderCommand::isText).findFirst().orElseThrow();
        assertEquals(stacks.color(), canvas.pixel(stacks.x(), stacks.y()),
            "状态层数文字必须位于 SVG 背景和图标上方");
    }

    @Test
    void laterSvgGeometryCoversEarlierBufferedGuiAndIsSubmittedBeforeReturning() {
        int foreground = 0xFFEEAA33;
        BufferedCanvas canvas = new BufferedCanvas(8, 8);
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.BASELINE, 0, 0, 8, 8, 0xFF224466),
            HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, 0, 0, 8, 8, foreground)
        );

        render(commands, canvas);

        assertEquals(foreground, canvas.pixel(4, 4),
            "较早的 GUI 缓冲不得在 SVG 前景之后提交；末批几何必须在后续 overlay 前完成");
    }

    private static void render(List<HudRenderCommand> commands, BufferedCanvas canvas) {
        BongHud.renderOrderedCommands(
            commands,
            SvgHudBackend.production()::handles,
            canvas.svgBuffer::add,
            canvas::renderGui,
            canvas::flush
        );
    }

    /** 模拟分层缓冲和立即绘制贴图，以最终像素而非 flush 调用次数验证遮挡契约。 */
    private static final class BufferedCanvas {
        private final int[][] pixels;
        private final List<HudRenderCommand> svgBuffer = new ArrayList<>();
        private final List<HudRenderCommand> guiBuffer = new ArrayList<>();

        private BufferedCanvas(int width, int height) {
            pixels = new int[height][width];
        }

        private void renderGui(HudRenderCommand command) {
            if (command.isTexturedRect()) {
                fill(command, ICON_COLOR);
            } else {
                guiBuffer.add(command);
            }
        }

        private void flush() {
            for (HudRenderCommand command : svgBuffer) {
                fill(command, command.color());
            }
            svgBuffer.clear();
            for (HudRenderCommand command : guiBuffer) {
                if (command.isText()) {
                    pixels[command.y()][command.x()] = command.color();
                } else {
                    fill(command, command.color());
                }
            }
            guiBuffer.clear();
        }

        private void fill(HudRenderCommand command, int color) {
            for (int y = command.y(); y < command.y() + command.height(); y++) {
                for (int x = command.x(); x < command.x() + command.width(); x++) {
                    pixels[y][x] = color;
                }
            }
        }

        private int pixel(int x, int y) {
            return pixels[y][x];
        }
    }
}
