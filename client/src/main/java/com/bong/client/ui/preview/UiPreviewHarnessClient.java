package com.bong.client.ui.preview;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/** 由环境变量显式激活的真实 UI 截图入口。 */
public final class UiPreviewHarnessClient {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-ui-preview");
    private static final String ENV_ENABLED = "BONG_UI_PREVIEW_HARNESS";
    private static final String ENV_CONFIG = "BONG_UI_PREVIEW_CONFIG";

    private UiPreviewHarnessClient() {
    }

    public static void install() {
        if (!"1".equals(System.getenv(ENV_ENABLED))) {
            return;
        }
        String configuredPath = System.getenv(ENV_CONFIG);
        if (configuredPath == null || configuredPath.isBlank()) {
            throw new IllegalStateException(ENV_CONFIG + " 未设置");
        }
        Path path = Path.of(configuredPath);
        if (!Files.isRegularFile(path)) {
            throw new IllegalStateException("UI preview 配置不存在: " + path.toAbsolutePath());
        }
        try {
            UiPreviewConfig config = UiPreviewConfig.parse(Files.readString(path));
            for (UiPreviewShot shot : config.screenshots()) {
                if (!UiPreviewScenes.isRegistered(shot.sceneId())) {
                    throw new IllegalArgumentException("未登记的 UI preview scene: " + shot.sceneId());
                }
            }
            UiPreviewSession session = new UiPreviewSession(
                config,
                new UiPreviewResultFile(config.outputDir())
            );
            ClientTickEvents.END_CLIENT_TICK.register(session::onTick);
            LOGGER.info("[ui-preview] installed config={} shots={} output={}",
                path.toAbsolutePath(), config.screenshots().size(), config.outputDir());
        } catch (IOException failure) {
            throw new IllegalStateException("读取 UI preview 配置失败: " + path.toAbsolutePath(), failure);
        }
    }
}
