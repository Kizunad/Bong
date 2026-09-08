package com.bong.client.menu;

import com.bong.client.BongClient;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.ConnectScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import net.minecraft.client.network.ServerAddress;
import net.minecraft.client.network.ServerInfo;
import net.minecraft.text.Text;

import java.nio.file.Path;

/** 场景跨菜单和原版连接页延续，网络生命周期仍由原版持有。 */
public final class MainMenuFlow {
    static final MainMenuBackdrop BACKDROP = new MainMenuBackdrop();
    private static boolean visible;
    private static boolean connecting;

    private MainMenuFlow() {
    }

    public static Path configPath() {
        return FabricLoader.getInstance().getConfigDir().resolve("bong-client.json");
    }

    static MainMenuConfig open() {
        visible = true;
        connecting = false;
        BACKDROP.returnToMenu();
        MainMenuConfig config = readConfig();
        MainMenuSound.configure(config != null && config.ambience());
        BACKDROP.motion(config == null || config.motion());
        return config;
    }

    static MainMenuConfig readConfig() {
        try {
            return MainMenuConfig.load(configPath());
        } catch (Exception failure) {
            BongClient.LOGGER.warn("无法读取客户端连接配置 {}", configPath(), failure);
            return null;
        }
    }

    static Text enter(MinecraftClient client) {
        if (connecting) {
            return Text.empty();
        }
        MainMenuConfig config = readConfig();
        if (config == null) {
            return Text.translatable("bong.menu.config_invalid");
        }
        if (config.serverAddress().isEmpty()) {
            return Text.translatable("bong.menu.config_missing");
        }
        connecting = true;
        BACKDROP.beginEntering();
        try {
            ConnectScreen.connect(new MainMenuScreen(), client, ServerAddress.parse(config.serverAddress()),
                new ServerInfo("末法残土", config.serverAddress(), false), false);
        } catch (RuntimeException failure) {
            connecting = false;
            BACKDROP.returnToMenu();
            BongClient.LOGGER.warn("无法开始连接", failure);
            return Text.translatable("bong.menu.connection_failed");
        }
        return Text.empty();
    }

    public static boolean shouldRenderBackground() {
        MinecraftClient client = MinecraftClient.getInstance();
        return visible && client != null && client.world == null;
    }

    public static boolean isConnectionParent(Screen parent) {
        return parent instanceof MainMenuScreen || parent instanceof TitleScreen || parent instanceof MultiplayerScreen;
    }

    public static void disconnected() {
        open();
    }

    public static boolean isConnecting() {
        return connecting;
    }

    public static void enteredWorld() {
        connecting = false;
        visible = false;
    }

    public static void retry() {
        open();
        MinecraftClient client = MinecraftClient.getInstance();
        Text failure = enter(client);
        if (!failure.getString().isEmpty()) {
            client.setScreen(new MainMenuScreen());
        }
    }

    public static void renderBackground(DrawContext context, int width, int height) {
        BACKDROP.render(context, width, height, width / 2, height / 2);
    }

    public static void renderConnection(DrawContext context, Screen screen, Text status, int mouseX, int mouseY) {
        BACKDROP.render(context, screen.width, screen.height, mouseX, mouseY);
        MainMenuBackdrop.renderConnectionText(context, screen, status, mouseX, mouseY);
    }

    public static void renderDisconnected(DrawContext context, Screen screen, Text reason, int mouseX, int mouseY) {
        BACKDROP.returnToMenu();
        BACKDROP.render(context, screen.width, screen.height, mouseX, mouseY);
        MainMenuBackdrop.renderDisconnectedText(context, screen, reason, mouseX, mouseY);
    }
}
