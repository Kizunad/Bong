package com.bong.client.ui.preview;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

/** UI 截图配置；只描述本地白名单场景和可核验的 viewport。 */
public record UiPreviewConfig(
    String outputDir,
    int waitClientTicks,
    int resizeTimeoutTicks,
    int settleTicks,
    boolean exitOnComplete,
    List<UiPreviewShot> screenshots
) {
    public UiPreviewConfig {
        if (outputDir == null || outputDir.isBlank()) {
            throw new IllegalArgumentException("output_dir 不能为空");
        }
        if (waitClientTicks <= 0 || resizeTimeoutTicks <= 0 || settleTicks < 1) {
            throw new IllegalArgumentException("等待 tick 必须为正数");
        }
        if (screenshots == null || screenshots.isEmpty()) {
            throw new IllegalArgumentException("screenshots 至少需要一个场景");
        }
        if (screenshots.stream().map(UiPreviewShot::name).distinct().count() != screenshots.size()) {
            throw new IllegalArgumentException("截图 name 必须唯一");
        }
        screenshots = List.copyOf(screenshots);
    }

    /** 从已读取的配置文本解析配置；文件系统 I/O 由外部入口负责。 */
    public static UiPreviewConfig parse(String content) {
        JsonObject root = JsonParser.parseString(Objects.requireNonNull(content, "配置文本不能为空"))
            .getAsJsonObject();
        if (!root.has("screenshots") || !root.get("screenshots").isJsonArray()) {
            throw new IllegalArgumentException("UI preview 配置缺少 screenshots 数组");
        }
        List<UiPreviewShot> shots = new ArrayList<>();
        for (JsonElement element : root.getAsJsonArray("screenshots")) {
            JsonObject shot = element.getAsJsonObject();
            shots.add(new UiPreviewShot(
                requiredString(shot, "name"),
                requiredString(shot, "scene_id"),
                requiredInt(shot, "framebuffer_width"),
                requiredInt(shot, "framebuffer_height"),
                requiredInt(shot, "gui_scale"),
                requiredInt(shot, "expected_logical_width"),
                requiredInt(shot, "expected_logical_height"),
                requiredString(shot, "expected_template_id")
            ));
        }
        return new UiPreviewConfig(
            optionalString(root, "output_dir").orElse("ui-preview-screenshots"),
            optionalInt(root, "wait_client_ticks").orElse(600),
            optionalInt(root, "resize_timeout_ticks").orElse(200),
            optionalInt(root, "settle_ticks").orElse(20),
            optionalBoolean(root, "exit_on_complete").orElse(true),
            shots
        );
    }

    private static String requiredString(JsonObject object, String key) {
        return optionalString(object, key)
            .orElseThrow(() -> new IllegalArgumentException("缺少字符串字段: " + key));
    }

    private static int requiredInt(JsonObject object, String key) {
        return optionalInt(object, key)
            .orElseThrow(() -> new IllegalArgumentException("缺少整数字段: " + key));
    }

    private static Optional<String> optionalString(JsonObject object, String key) {
        return Optional.ofNullable(object.get(key))
            .filter(JsonElement::isJsonPrimitive)
            .map(JsonElement::getAsString);
    }

    private static Optional<Integer> optionalInt(JsonObject object, String key) {
        return Optional.ofNullable(object.get(key))
            .filter(JsonElement::isJsonPrimitive)
            .map(JsonElement::getAsInt);
    }

    private static Optional<Boolean> optionalBoolean(JsonObject object, String key) {
        return Optional.ofNullable(object.get(key))
            .filter(JsonElement::isJsonPrimitive)
            .map(JsonElement::getAsBoolean);
    }
}
