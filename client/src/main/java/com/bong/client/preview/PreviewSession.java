package com.bong.client.preview;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.util.ScreenshotRecorder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;

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
        if (client.player == null || client.player.networkHandler == null) {
            LOGGER.error("[preview] client.player / networkHandler == null in SETUP_SHOT");
            requestStop(client);
            return;
        }
        // 关 HUD 避免聊天/toast 字遮挡
        client.options.hudHidden = true;
        if (client.getToastManager() != null) {
            client.getToastManager().clear();
        }
        // 走 server-side authoritative tp（/preview_tp 原生命令）—— 避免
        // multi-player anti-cheat 把 client.setPos 远距离 force-sync 回原位。
        // server 收到 brigadier 命令后 cmd::dev::preview_tp::handle_preview_tp
        // 解析 → emit PreviewTeleportRequested → preview module system 改写
        // Position/Look/HeadYaw → server 主动下发 PlayerPosLook 同步 client。
        //
        // 历史：首版用 `!preview-tp` chat 命令走 chat_collector 解析；main 上
        // commit 162bc974 把所有 `!`-prefix dev 命令迁到原生 `/` 命令树
        // (valence_command brigadier)，本 client 跟随迁移。命令名用下划线
        // `preview_tp` 而非短横（valence brigadier literal 不允许 `-`）。
        //
        // 需要 server 启动设 BONG_PREVIEW_MODE=1，未设的话 preview module 不
        // 注册 handle_preview_teleport system，event 没人消费 player 不动。
        String cmd = String.format(
                "preview_tp %.3f %.3f %.3f %.1f %.1f",
                shot.tp()[0], shot.tp()[1], shot.tp()[2], shot.yaw(), shot.pitch());
        client.player.networkHandler.sendCommand(cmd);
        LOGGER.info("[preview] shot[{}/{}] '{}' sent {}",
                currentShotIdx + 1, config.screenshots().size(),
                shot.name(), cmd);
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
