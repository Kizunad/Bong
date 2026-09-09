package com.bong.client.menu;

import com.google.common.net.HostAndPort;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

/** 登录前的本地配置，不依赖服务器下发资源或世界状态。 */
public record MainMenuConfig(String serverAddress, boolean motion, boolean ambience) {
    public MainMenuConfig {
        if (serverAddress == null) {
            throw new IllegalArgumentException("serverAddress 必须是字符串");
        }
        serverAddress = serverAddress.strip();
        if (!serverAddress.isEmpty()) {
            HostAndPort parsed = HostAndPort.fromString(serverAddress).requireBracketsForIPv6();
            if (parsed.getHost().isBlank() || serverAddress.chars().anyMatch(Character::isWhitespace)
                || serverAddress.contains("/") || serverAddress.contains("@")
                || serverAddress.endsWith(":")) {
                throw new IllegalArgumentException("服务器地址应为主机名或 host:port");
            }
            if (parsed.hasPort() && parsed.getPort() < 1) {
                throw new IllegalArgumentException("端口必须介于 1 和 65535 之间");
            }
        }
    }

    public static MainMenuConfig load(Path path) throws IOException {
        if (!Files.exists(path)) {
            Files.createDirectories(path.toAbsolutePath().getParent());
            JsonObject defaults = new JsonObject();
            defaults.addProperty("serverAddress", "");
            defaults.addProperty("motion", true);
            defaults.addProperty("ambience", true);
            try {
                Files.writeString(path, new GsonBuilder().setPrettyPrinting().create().toJson(defaults) + "\n",
                    StandardCharsets.UTF_8, StandardOpenOption.CREATE_NEW);
            } catch (FileAlreadyExistsException ignored) {
                // 另一个客户端抢先创建时，读取其配置，不覆盖。
            }
        }
        return parse(Files.readString(path, StandardCharsets.UTF_8));
    }

    static MainMenuConfig parse(String source) {
        JsonObject json = JsonParser.parseString(source).getAsJsonObject();
        if (!json.has("serverAddress") || !json.get("serverAddress").isJsonPrimitive()
            || !json.getAsJsonPrimitive("serverAddress").isString()) {
            throw new IllegalArgumentException("serverAddress 必须是字符串");
        }
        return new MainMenuConfig(json.get("serverAddress").getAsString(),
            booleanValue(json, "motion"), booleanValue(json, "ambience"));
    }

    private static boolean booleanValue(JsonObject json, String key) {
        if (!json.has(key)) {
            return true;
        }
        if (!json.get(key).isJsonPrimitive() || !json.getAsJsonPrimitive(key).isBoolean()) {
            throw new IllegalArgumentException(key + " 必须是布尔值");
        }
        return json.get(key).getAsBoolean();
    }
}
