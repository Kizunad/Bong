package com.bong.client.ui.preview;

import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.adapter.owo.OwoXmlTemplateRegistry;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.util.ScreenshotRecorder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.Objects;

/** 真实 Fabric/owo UI 截图状态机。 */
final class UiPreviewSession {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-ui-preview");

    private enum Phase {
        WAIT_CLIENT,
        CONFIGURE_VIEWPORT,
        WAIT_VIEWPORT,
        OPEN_SCREEN,
        WAIT_SCREEN,
        SETTLE,
        SHOOT,
        CLOSE_SCREEN,
        FINISHED
    }

    private final UiPreviewConfig config;
    private final UiPreviewArtifactSink artifacts;
    private Phase phase = Phase.WAIT_CLIENT;
    private int phaseTicks;
    private int totalTicks;
    private int shotIndex;
    private Screen openedScreen;
    private UiPreviewScene openedScene;
    private boolean stopRequested;

    UiPreviewSession(UiPreviewConfig config, UiPreviewArtifactSink artifacts) {
        this.config = Objects.requireNonNull(config, "config 不能为空");
        this.artifacts = Objects.requireNonNull(artifacts, "artifacts 不能为空");
        try {
            artifacts.begin();
        } catch (IOException e) {
            throw new IllegalStateException("无法初始化 UI preview 产物仓储", e);
        }
    }

    void onTick(MinecraftClient client) {
        if (stopRequested) {
            return;
        }
        totalTicks++;
        phaseTicks++;
        try {
            step(client);
        } catch (RuntimeException | IOException failure) {
            recordFailureAndStop(client, cleanupAfterFailure(client, failure));
        }
    }

    private void step(MinecraftClient client) throws IOException {
        switch (phase) {
            case WAIT_CLIENT -> waitClient(client);
            case CONFIGURE_VIEWPORT -> configureViewport(client);
            case WAIT_VIEWPORT -> waitViewport(client);
            case OPEN_SCREEN -> openScreen(client);
            case WAIT_SCREEN -> waitScreen(client);
            case SETTLE -> settle();
            case SHOOT -> shoot(client);
            case CLOSE_SCREEN -> closeScreen(client);
            case FINISHED -> finish(client);
        }
    }

    private void waitClient(MinecraftClient client) {
        boolean ready = client.getWindow() != null
            && client.getFramebuffer() != null
            && client.getWindow().getFramebufferWidth() > 0
            && templatesLoaded();
        if (ready && phaseTicks >= 5) {
            advance(Phase.CONFIGURE_VIEWPORT);
            return;
        }
        if (phaseTicks > config.waitClientTicks()) {
            throw new IllegalStateException("等待 Minecraft client 初始化超时");
        }
    }

    private void configureViewport(MinecraftClient client) {
        if (shotIndex >= config.screenshots().size()) {
            advance(Phase.FINISHED);
            return;
        }
        UiPreviewShot shot = currentShot();
        if (client.getWindow().isFullscreen()) {
            client.getWindow().toggleFullscreen();
        }
        client.getWindow().setWindowedSize(shot.framebufferWidth(), shot.framebufferHeight());
        client.onResolutionChanged();
        LOGGER.info(
            "[ui-preview] configuring '{}' framebuffer={}x{} scale={} expected logical={}x{}",
            shot.name(), shot.framebufferWidth(), shot.framebufferHeight(), shot.guiScale(),
            shot.expectedLogicalWidth(), shot.expectedLogicalHeight());
        advance(Phase.WAIT_VIEWPORT);
    }

    private void waitViewport(MinecraftClient client) {
        UiPreviewShot shot = currentShot();
        boolean framebufferMatches = client.getWindow().getFramebufferWidth() == shot.framebufferWidth()
            && client.getWindow().getFramebufferHeight() == shot.framebufferHeight();
        if (framebufferMatches) {
            // 必须先等物理窗口到位；Minecraft 会按旧窗口尺寸压低暂时不可用的 GUI scale。
            client.options.getGuiScale().setValue(shot.guiScale());
            client.onResolutionChanged();
        }
        boolean logicalMatches = client.getWindow().getScaledWidth() == shot.expectedLogicalWidth()
            && client.getWindow().getScaledHeight() == shot.expectedLogicalHeight();
        if (framebufferMatches && logicalMatches) {
            advance(Phase.OPEN_SCREEN);
            return;
        }
        if (phaseTicks > config.resizeTimeoutTicks()) {
            throw new IllegalStateException(String.format(
                "viewport 未生效: framebuffer=%dx%d logical=%dx%d",
                client.getWindow().getFramebufferWidth(), client.getWindow().getFramebufferHeight(),
                client.getWindow().getScaledWidth(), client.getWindow().getScaledHeight()));
        }
    }

    private void openScreen(MinecraftClient client) {
        UiPreviewShot shot = currentShot();
        openedScene = UiPreviewScenes.require(shot.sceneId());
        openedScene.installFixture();
        openedScreen = openedScene.createScreen();
        client.setScreen(openedScreen);
        advance(Phase.WAIT_SCREEN);
    }

    private void waitScreen(MinecraftClient client) {
        if (client.currentScreen == openedScreen) {
            if (openedScene.initializationFailed(openedScreen)) {
                throw new IllegalStateException("owo Screen 初始化失败，禁止生成截图证据");
            }
            if (!openedScene.isReady(openedScreen)) {
                if (phaseTicks > config.resizeTimeoutTicks()) {
                    throw new IllegalStateException("owo Screen adapter 初始化超时");
                }
                return;
            }
            String actualTemplate = openedScene.selectedTemplateId(openedScreen);
            if (!currentShot().expectedTemplateId().equals(actualTemplate)) {
                throw new IllegalStateException(
                    "布局模板错误: expected=" + currentShot().expectedTemplateId()
                        + ", actual=" + actualTemplate);
            }
            advance(Phase.SETTLE);
            return;
        }
        if (phaseTicks > config.resizeTimeoutTicks()) {
            throw new IllegalStateException("等待 UI Screen 打开超时");
        }
    }

    private void settle() {
        if (phaseTicks >= config.settleTicks()) {
            advance(Phase.SHOOT);
        }
    }

    private void shoot(MinecraftClient client) throws IOException {
        UiPreviewShot shot = currentShot();
        // 等真实渲染帧完成后再检查输入命中；owo scrollbar 的 hit region 在 draw 时初始化。
        openedScene.validateGeometry(openedScreen, shot);
        String metadata = "scene_id=" + shot.sceneId() + "\n"
            + "framebuffer=" + client.getWindow().getFramebufferWidth() + "x"
            + client.getWindow().getFramebufferHeight() + "\n"
            + "logical_viewport=" + client.getWindow().getScaledWidth() + "x"
            + client.getWindow().getScaledHeight() + "\n"
            + "gui_scale=" + client.getWindow().getScaleFactor() + "\n"
            + "template_id=" + openedScene.selectedTemplateId(openedScreen) + "\n";
        try (NativeImage image = ScreenshotRecorder.takeScreenshot(client.getFramebuffer())) {
            String imagePath = artifacts.writeShot(shot.name(), image, metadata);
            LOGGER.info("[ui-preview] saved {}", imagePath);
        }
        advance(Phase.CLOSE_SCREEN);
    }

    private void closeScreen(MinecraftClient client) {
        ScreenTransitionController.cancelAndClose(client);
        openedScene.cleanup();
        openedScene = null;
        openedScreen = null;
        shotIndex++;
        advance(Phase.CONFIGURE_VIEWPORT);
    }

    private void finish(MinecraftClient client) throws IOException {
        if (phaseTicks < 5) {
            return;
        }
        artifacts.passed(shotIndex);
        LOGGER.info("[ui-preview] completed {} screenshots in {} ticks", shotIndex, totalTicks);
        CompletionDecision completion = completionDecision(config.exitOnComplete());
        stopRequested = completion.stopTicks();
        if (completion.stopClient()) {
            client.scheduleStop();
        }
    }

    private void recordFailureAndStop(MinecraftClient client, Throwable failure) {
        try {
            artifacts.failed(shotIndex, phase.name(), shotName(), failure);
        } catch (IOException resultFailure) {
            failure.addSuppressed(resultFailure);
        }
        LOGGER.error(
            "[ui-preview] failed phase={} shot={} completed={}",
            phase, shotName(), shotIndex, failure
        );
        stopRequested = true;
        client.scheduleStop();
    }

    private Throwable cleanupAfterFailure(MinecraftClient client, Throwable failure) {
        // Screen 关闭失败不能阻断 fixture store 清理，两个生命周期阶段必须分别尝试。
        return attachCleanupFailures(
            failure,
            () -> {
                if (openedScreen != null) {
                    ScreenTransitionController.cancelAndClose(client);
                }
            },
            () -> {
                if (openedScene != null) {
                    openedScene.cleanup();
                }
            }
        );
    }

    static Throwable attachCleanupFailures(Throwable primary, Runnable... cleanups) {
        Objects.requireNonNull(cleanups, "cleanups 不能为空");
        Throwable result = Objects.requireNonNull(primary, "primary 不能为空");
        for (Runnable cleanup : cleanups) {
            result = attachCleanupFailure(result, cleanup);
        }
        return result;
    }

    static Throwable attachCleanupFailure(Throwable primary, Runnable cleanup) {
        Objects.requireNonNull(primary, "primary 不能为空");
        Objects.requireNonNull(cleanup, "cleanup 不能为空");
        try {
            cleanup.run();
        } catch (Throwable cleanupFailure) {
            if (cleanupFailure != primary) {
                primary.addSuppressed(cleanupFailure);
            }
        }
        return primary;
    }

    static CompletionDecision completionDecision(boolean exitOnComplete) {
        return new CompletionDecision(true, exitOnComplete);
    }

    private UiPreviewShot currentShot() {
        return config.screenshots().get(shotIndex);
    }

    private String shotName() {
        return shotIndex < config.screenshots().size()
            ? config.screenshots().get(shotIndex).name()
            : "complete";
    }

    private void advance(Phase next) {
        phase = next;
        phaseTicks = 0;
    }

    private static boolean templatesLoaded() {
        try {
            OwoXmlTemplateRegistry.production().require("craft");
            OwoXmlTemplateRegistry.production().require("craft-compact");
            OwoXmlTemplateRegistry.production().require("terminate");
            OwoXmlTemplateRegistry.production().require("coffin-menu");
            OwoXmlTemplateRegistry.production().require("repair");
            OwoXmlTemplateRegistry.production().require("death");
            OwoXmlTemplateRegistry.production().require("forge-carrier");
            return true;
        } catch (IllegalStateException notLoadedYet) {
            return false;
        }
    }

    record CompletionDecision(boolean stopTicks, boolean stopClient) {
    }
}
