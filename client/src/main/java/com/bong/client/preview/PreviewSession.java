package com.bong.client.preview;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.util.ScreenshotRecorder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.util.Arrays;

/**
 * Preview 截图状态机，由 {@link PreviewHarnessClient#install()} 注册到
 * {@code ClientTickEvents.END_CLIENT_TICK}。
 *
 * 状态流转:
 *   WAIT_WORLD  → WAIT_CHUNKS → SETUP_SHOT → SETTLE → SHOOT
 *   ↑                            ↓
 *   └────── 还有 shot 时回 SETUP_SHOT 拍下一张
 *                                ↓ (全部拍完)
 *                              FINISHED → scheduleStop
 *
 * 任一阶段超时 / 异常都会调用 {@code client.scheduleStop()} 让进程退出，
 * 让 CI 拿非零 exit code。配合 workflow timeout 兜底。
 */
public final class PreviewSession {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-preview");

    private enum Phase {
        WAIT_WORLD,
        WAIT_CHUNKS,
        SETUP_SHOT,
        SETTLE,
        SHOOT,
        FINISHED,
    }

    private final PreviewConfig config;
    private final File outDir;

    private Phase phase = Phase.WAIT_WORLD;
    private int phaseTicks = 0;
    private int totalTicks = 0;
    private int currentShotIdx = 0;
    private boolean stopRequested = false;
    private int shotsTaken = 0;

    public PreviewSession(PreviewConfig config) {
        this.config = config;
        this.outDir = new File(config.outputDir()).getAbsoluteFile();
        if (!outDir.exists() && !outDir.mkdirs()) {
            throw new IllegalStateException(
                    "无法创建截图输出目录: " + outDir);
        }
    }

    public int shotsTaken() {
        return shotsTaken;
    }

    public void onTick(MinecraftClient client) {
        if (stopRequested) return;
        totalTicks++;
        phaseTicks++;

        // 防 WSLg / xvfb 焦点切换弹 GameMenuScreen 遮住截图
        client.options.pauseOnLostFocus = false;
        if (client.currentScreen != null && phase != Phase.WAIT_WORLD) {
            client.setScreen(null);
        }

        try {
            step(client);
        } catch (Exception e) {
            LOGGER.error("[preview] state machine crashed in phase {}", phase, e);
            requestStop(client);
        }
    }

    private void step(MinecraftClient client) {
        switch (phase) {
            case WAIT_WORLD -> stepWaitWorld(client);
            case WAIT_CHUNKS -> stepWaitChunks();
            case SETUP_SHOT -> stepSetupShot(client);
            case SETTLE -> stepSettle();
            case SHOOT -> stepShoot(client);
            case FINISHED -> stepFinished(client);
        }
    }

    private void stepWaitWorld(MinecraftClient client) {
        if (client.world != null && client.player != null && client.player.age > 5) {
            LOGGER.info("[preview] world ready (player age={}), advancing to WAIT_CHUNKS",
                    client.player.age);
            advance(Phase.WAIT_CHUNKS);
            return;
        }
        if (totalTicks > config.waitWorldTicks()) {
            LOGGER.error("[preview] timeout({} ticks)等待 world 加载，"
                    + "server 没起来或 client 无法连接 — 退出",
                    config.waitWorldTicks());
            requestStop(client);
        }
    }

    private void stepWaitChunks() {
        if (phaseTicks >= config.waitChunksTicks()) {
            LOGGER.info("[preview] WAIT_CHUNKS done after {} ticks, going SETUP_SHOT",
                    phaseTicks);
            advance(Phase.SETUP_SHOT);
        }
    }

    private void stepSetupShot(MinecraftClient client) {
        if (currentShotIdx >= config.screenshots().size()) {
            advance(Phase.FINISHED);
            return;
        }
        PreviewShot shot = config.screenshots().get(currentShotIdx);
        if (client.player == null) {
            LOGGER.error("[preview] client.player == null in SETUP_SHOT — 异常退出");
            requestStop(client);
            return;
        }
        // 关 HUD 避免聊天/toast 字遮挡。FP 视角 + 关 HUD 对俯视/等角都合适
        client.options.hudHidden = true;
        if (client.getToastManager() != null) {
            client.getToastManager().clear();
        }
        client.player.setPosition(shot.tp()[0], shot.tp()[1], shot.tp()[2]);
        client.player.setYaw(shot.yaw());
        client.player.setPitch(shot.pitch());
        LOGGER.info("[preview] shot[{}/{}] '{}' setup tp={} yaw={} pitch={}",
                currentShotIdx + 1, config.screenshots().size(),
                shot.name(), Arrays.toString(shot.tp()), shot.yaw(), shot.pitch());
        advance(Phase.SETTLE);
    }

    private void stepSettle() {
        if (phaseTicks >= config.settleTicks()) {
            advance(Phase.SHOOT);
        }
    }

    private void stepShoot(MinecraftClient client) {
        PreviewShot shot = config.screenshots().get(currentShotIdx);
        String fileName = "preview-" + shot.name() + ".png";
        try {
            NativeImage img = ScreenshotRecorder.takeScreenshot(client.getFramebuffer());
            File dst = new File(outDir, fileName);
            img.writeTo(dst);
            img.close();
            shotsTaken++;
            LOGGER.info("[preview] saved {} ({} bytes)", dst.getAbsolutePath(),
                    dst.length());
        } catch (IOException e) {
            LOGGER.error("[preview] screenshot 写入失败: {}", fileName, e);
        }
        currentShotIdx++;
        advance(Phase.SETUP_SHOT);
    }

    private void stepFinished(MinecraftClient client) {
        if (phaseTicks < 5) return;
        LOGGER.info("[preview] all {} shots done (taken={}), scheduling stop",
                config.screenshots().size(), shotsTaken);
        if (config.exitOnComplete()) {
            requestStop(client);
        }
    }

    private void advance(Phase next) {
        phase = next;
        phaseTicks = 0;
    }

    private void requestStop(MinecraftClient client) {
        if (stopRequested) return;
        stopRequested = true;
        LOGGER.info("[preview] requestStop: phase={} totalTicks={} shotsTaken={}",
                phase, totalTicks, shotsTaken);
        client.scheduleStop();
    }
}
