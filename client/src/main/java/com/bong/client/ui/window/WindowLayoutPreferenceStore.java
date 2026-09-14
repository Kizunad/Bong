package com.bong.client.ui.window;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.LinkedHashMap;
import java.util.Map;

/** 仅存本地呈现偏好，不持久化窗口实例、业务身份或权威快照。 */
public final class WindowLayoutPreferenceStore {
    private final Path path;
    private final Map<String, Preference> windows = new LinkedHashMap<>();
    private final Map<String, HudPreference> hud = new LinkedHashMap<>();
    private String background = "cosmos";
    private boolean motion = true;

    public WindowLayoutPreferenceStore(Path path) { this.path = path; }
    public Preference window(String type) { return windows.get(type); }
    public String background() { return background; }
    public boolean motion() { return motion; }
    public void background(String id) { background = id; }
    public void motion(boolean enabled) { motion = enabled; }
    public HudPreference hud(String id) { return hud.getOrDefault(id, HudPreference.DEFAULT); }
    public void hud(String id, HudPreference value) { hud.put(id, value); }
    public void resetLayouts() { windows.clear(); hud.clear(); }

    public void remember(UiWindowManager.WindowState state) {
        windows.put(state.key().windowType(), new Preference(state.desiredBounds(), state.pinned(), state.minimized()));
    }

    public void load() throws IOException {
        if (!Files.exists(path)) return;
        if (Files.size(path) > 128 * 1024) throw new IOException("窗口配置文件过大");
        try {
            JsonObject root = JsonParser.parseString(Files.readString(path, StandardCharsets.UTF_8)).getAsJsonObject();
            if (integer(root, "version") != 1) return;
            Map<String, Preference> loaded = new LinkedHashMap<>();
            for (var entry : root.getAsJsonObject("windows").entrySet()) {
                var value = entry.getValue().getAsJsonObject();
                var rect = new UiWindowManager.Rect(integer(value, "x"), integer(value, "y"),
                    integer(value, "width"), integer(value, "height"));
                loaded.put(entry.getKey(), new Preference(rect, bool(value, "pinned"), bool(value, "minimized")));
            }
            String selected = root.get("background").getAsString();
            boolean enabled = bool(root, "motion");
            Map<String, HudPreference> loadedHud = new LinkedHashMap<>();
            if (root.has("hud")) {
                for (var entry : root.getAsJsonObject("hud").entrySet()) {
                    var value = entry.getValue().getAsJsonObject();
                    loadedHud.put(entry.getKey(), new HudPreference(value.get("offset_x").getAsDouble(),
                        value.get("offset_y").getAsDouble(), value.get("scale").getAsDouble(), bool(value, "visible")));
                }
            }
            windows.clear();
            windows.putAll(loaded);
            hud.clear();
            hud.putAll(loadedHud);
            background = selected;
            motion = enabled;
        } catch (RuntimeException invalid) {
            throw new IOException("窗口配置无效，使用默认布局", invalid);
        }
    }

    public void save() throws IOException {
        JsonObject root = new JsonObject();
        root.addProperty("version", 1);
        root.addProperty("background", background);
        root.addProperty("motion", motion);
        JsonObject layouts = new JsonObject();
        windows.forEach((type, preference) -> {
            JsonObject value = new JsonObject();
            value.addProperty("x", preference.bounds.x());
            value.addProperty("y", preference.bounds.y());
            value.addProperty("width", preference.bounds.width());
            value.addProperty("height", preference.bounds.height());
            value.addProperty("pinned", preference.pinned);
            value.addProperty("minimized", preference.minimized);
            layouts.add(type, value);
        });
        root.add("windows", layouts);
        JsonObject hudLayouts = new JsonObject();
        hud.forEach((id, preference) -> {
            JsonObject value = new JsonObject();
            value.addProperty("offset_x", preference.offsetX());
            value.addProperty("offset_y", preference.offsetY());
            value.addProperty("scale", preference.scale());
            value.addProperty("visible", preference.visible());
            hudLayouts.add(id, value);
        });
        root.add("hud", hudLayouts);
        Files.createDirectories(path.toAbsolutePath().getParent());
        Path temporary = path.resolveSibling(path.getFileName() + ".tmp");
        Files.writeString(temporary, new GsonBuilder().setPrettyPrinting().create().toJson(root) + "\n",
            StandardCharsets.UTF_8);
        try {
            Files.move(temporary, path, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } catch (java.nio.file.AtomicMoveNotSupportedException unsupported) {
            Files.move(temporary, path, StandardCopyOption.REPLACE_EXISTING);
        }
    }

    private static int integer(JsonObject value, String key) {
        var number = value.getAsJsonPrimitive(key);
        if (!number.isNumber()) throw new IllegalArgumentException("窗口尺寸必须为整数");
        return number.getAsBigDecimal().intValueExact();
    }

    private static boolean bool(JsonObject value, String key) {
        var flag = value.getAsJsonPrimitive(key);
        if (!flag.isBoolean()) throw new IllegalArgumentException("窗口开关必须为布尔值");
        return flag.getAsBoolean();
    }

    public record Preference(UiWindowManager.Rect bounds, boolean pinned, boolean minimized) {}

    /** 位移按屏幕比例保存，缩放以屏幕中心为基准；默认值严格保留原始 HUD 布局。 */
    public record HudPreference(double offsetX, double offsetY, double scale, boolean visible) {
        public static final HudPreference DEFAULT = new HudPreference(0, 0, 1, true);
        public HudPreference {
            if (!Double.isFinite(offsetX) || !Double.isFinite(offsetY) || !Double.isFinite(scale)
                || Math.abs(offsetX) > 4 || Math.abs(offsetY) > 4 || scale < .25 || scale > 4) {
                throw new IllegalArgumentException("HUD 布局超出有效范围");
            }
        }
        public HudPreference visible(boolean enabled) { return new HudPreference(offsetX, offsetY, scale, enabled); }
        public double translationX(int width) { return width * (offsetX + (1 - scale) / 2); }
        public double translationY(int height) { return height * (offsetY + (1 - scale) / 2); }
    }
}
