package com.bong.client;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.StatusEffectHudPlanner;
import com.bong.client.hud.svg.SvgHudBackend;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import org.joml.Matrix3f;
import org.joml.Matrix4f;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

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

    @ParameterizedTest
    @ValueSource(booleans = {false, true})
    void failedRendererFlushesPendingGeometryBeforePropagating(boolean backend) {
        BufferedCanvas canvas = new BufferedCanvas(8, 8);
        HudRenderLayer layer = backend ? HudRenderLayer.STATUS_EFFECTS : HudRenderLayer.BASELINE;
        HudRenderCommand command = HudRenderCommand.rect(layer, 0, 0, 8, 8, 0xFF224466);
        RuntimeException renderFailure = new IllegalStateException("绘制中断");
        java.util.function.Consumer<HudRenderCommand> failingRenderer = rendered -> {
            if (backend) {
                canvas.svgBuffer.add(rendered);
            } else {
                canvas.renderGui(rendered);
            }
            throw renderFailure;
        };

        RuntimeException thrown = assertThrows(RuntimeException.class, () -> BongHud.renderOrderedCommands(
            List.of(command),
            SvgHudBackend.production()::handles,
            failingRenderer,
            failingRenderer,
            canvas::flush
        ));

        assertSame(renderFailure, thrown, "收尾后必须传播原始绘制异常");
        assertEquals(command.color(), canvas.pixel(4, 4), "异常前入队的几何必须完成末批提交");
        canvas.renderGui(HudRenderCommand.texture(layer, "next-hud", 0, 0, 8, 8, 0xFFFFFFFF));
        canvas.flush();
        assertEquals(ICON_COLOR, canvas.pixel(4, 4), "残留缓冲不得在后续 HUD 绘制后覆盖它");
    }

    @Test
    void failedCleanupIsSuppressedByTheOriginalRenderFailure() {
        RuntimeException renderFailure = new IllegalStateException("绘制中断");
        Error cleanupFailure = new AssertionError("末批提交失败");

        RuntimeException thrown = assertThrows(RuntimeException.class, () -> BongHud.renderOrderedCommands(
            List.of(HudRenderCommand.text(HudRenderLayer.BASELINE, "文字", 0, 0, 0xFFFFFFFF)),
            command -> false,
            command -> { },
            command -> { throw renderFailure; },
            () -> { throw cleanupFailure; }
        ));

        assertSame(renderFailure, thrown, "清理异常不得掩盖最初的绘制失败");
        assertArrayEquals(new Throwable[]{cleanupFailure}, thrown.getSuppressed(),
            "清理失败必须作为 suppressed 保留，供诊断使用");
    }

    @ParameterizedTest
    @ValueSource(booleans = {false, true})
    void flushFailureRemainsPrimaryAtBatchSwitchAndAtEnd(boolean backend) {
        Error flushFailure = new AssertionError("缓冲提交失败");

        Error thrown = assertThrows(Error.class, () -> BongHud.renderOrderedCommands(
            List.of(HudRenderCommand.rect(HudRenderLayer.BASELINE, 0, 0, 1, 1, 0xFFFFFFFF)),
            command -> backend,
            command -> { },
            command -> { },
            () -> { throw flushFailure; }
        ));

        assertSame(flushFailure, thrown, "flush 自身失败必须原样传播，重复失败不得触发自抑制异常");
        assertEquals(0, thrown.getSuppressed().length, "同一个异常对象不得添加为自己的 suppressed");
    }

    @ParameterizedTest
    @ValueSource(booleans = {false, true})
    void scaledTextRestoresCallerMatricesAfterSuccessOrFailure(boolean fail) {
        Error renderFailure = new AssertionError("文字绘制失败");
        Matrix4f callerPosition = new Matrix4f().translation(7, 11, 2);
        HudRenderCommand command = HudRenderCommand.scaledText(
            HudRenderLayer.BASELINE, "缩放文字", 9, 13, 0xFFFFFFFF, 0.75
        );
        DrawContext context = new DrawContext(null, null) {
            @Override
            public int drawTextWithShadow(TextRenderer renderer, String text, int x, int y, int color) {
                assertEquals(new Matrix4f(callerPosition).translate(9, 13, 0).scale(0.75f, 0.75f, 1),
                    getMatrices().peek().getPositionMatrix(), "文字绘制时仍须应用原有平移和缩放");
                if (fail) {
                    throw renderFailure;
                }
                return 0;
            }
        };
        var matrices = context.getMatrices();
        matrices.translate(7, 11, 2);
        Matrix3f callerNormal = new Matrix3f(matrices.peek().getNormalMatrix());

        if (fail) {
            Error thrown = assertThrows(Error.class, () -> BongHud.renderScaledText(context, null, command));
            assertSame(renderFailure, thrown, "恢复矩阵后仍须传播原始文字绘制异常");
        } else {
            BongHud.renderScaledText(context, null, command);
        }

        assertEquals(callerPosition, matrices.peek().getPositionMatrix(), "后续 HUD 必须沿用调用方的位置矩阵");
        assertEquals(callerNormal, matrices.peek().getNormalMatrix(), "缩放不得残留在调用方的法线矩阵中");
        assertTrue(matrices.isEmpty(), "无论绘制是否成功，矩阵栈都必须恢复为调用前的深度");
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
