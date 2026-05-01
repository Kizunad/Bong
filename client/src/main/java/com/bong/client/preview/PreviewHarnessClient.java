package com.bong.client.preview;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * 通过环境变量激活的 worldgen-snapshot 截图 harness。
 *
 * 激活方式（典型 CI workflow 或 scripts/preview/run-client-headless.sh wrapper）：
 *   BONG_PREVIEW_HARNESS=1                                  必填，设为 "1" 才激活
 *   BONG_PREVIEW_CONFIG=/abs/path/to/preview-harness.json   可选，默认查找
 *                                                           CWD 下的 preview-harness.json
 *
 * 环境变量优于 -D system property：gradle runClient → JavaExec 子进程，env var
 * 默认继承（参照 client/src/main/java/com/bong/client/weapon/WeaponScreenshotHarness.java
 * 同模式）。
 *
 * 注意：未设激活变量时 install() 直接 no-op 返回，对普通 runClient 0 影响。
 */
public final class PreviewHarnessClient {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-preview");
    private static final String ENV_ENABLED = "BONG_PREVIEW_HARNESS";
    private static final String ENV_CONFIG = "BONG_PREVIEW_CONFIG";
    private static final String DEFAULT_CONFIG_FILE = "preview-harness.json";

    private PreviewHarnessClient() {}

    public static void install() {
        String enabled = System.getenv(ENV_ENABLED);
        if (enabled == null || !enabled.equals("1")) {
            return;
        }

        Path configPath = resolveConfigPath();
        if (!Files.isRegularFile(configPath)) {
            LOGGER.error("[preview] {} 指向的配置文件不存在: {}",
                    ENV_CONFIG, configPath.toAbsolutePath());
            return;
        }

        PreviewConfig config;
        try {
            config = PreviewConfig.load(configPath);
        } catch (IOException | RuntimeException e) {
            LOGGER.error("[preview] 解析配置文件失败: {}", configPath.toAbsolutePath(), e);
            return;
        }

        PreviewSession session;
        try {
            session = new PreviewSession(config);
        } catch (RuntimeException e) {
            LOGGER.error("[preview] 创建 session 失败（输出目录无法创建？）", e);
            return;
        }

        ClientTickEvents.END_CLIENT_TICK.register(session::onTick);
        LOGGER.info(
                "[preview] PreviewHarnessClient installed | config={} screenshots={} output={}",
                configPath.toAbsolutePath(), config.screenshots().size(), config.outputDir());
    }

    private static Path resolveConfigPath() {
        String env = System.getenv(ENV_CONFIG);
        if (env != null && !env.isBlank()) {
            return Path.of(env);
        }
        return Path.of(DEFAULT_CONFIG_FILE);
    }
}
