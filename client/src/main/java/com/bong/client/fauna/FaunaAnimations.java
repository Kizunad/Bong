package com.bong.client.fauna;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** 从随包动画读取动作清单和时长，让控制器只播放真实存在的轨道。 */
public final class FaunaAnimations {
    private static final Map<String, Profile> CACHE = new ConcurrentHashMap<>();

    public record Clip(String name, int ticks, boolean loop) {}

    public record Profile(String path, Map<String, Clip> clips, Clip idle, Clip walk, Clip run) {
        public Clip find(String name) {
            Clip exact = clips.get(name);
            if (exact != null) return exact;
            String prefix = path.endsWith("_core") ? "core_" : path.endsWith("_shard") ? "shard_" : "";
            return clips.values().stream().filter(clip -> shortName(clip.name()).equals(name)
                || (!prefix.isEmpty() && shortName(clip.name()).equals(prefix + name)))
                .findFirst().orElse(null);
        }
    }

    private FaunaAnimations() {}

    public static Profile load(String path) {
        return CACHE.computeIfAbsent(path, FaunaAnimations::read);
    }

    public static String shortName(String name) {
        return name.substring(name.lastIndexOf('.') + 1);
    }

    private static Profile read(String path) {
        String resource = "/assets/bong/animations/" + path + ".animation.json";
        try (var stream = FaunaAnimations.class.getResourceAsStream(resource)) {
            if (stream == null) throw new IllegalStateException("缺少生物动画：" + resource);
            JsonObject animations = JsonParser.parseReader(new InputStreamReader(stream, StandardCharsets.UTF_8))
                .getAsJsonObject().getAsJsonObject("animations");
            Map<String, Clip> clips = new LinkedHashMap<>();
            animations.entrySet().forEach(entry -> {
                JsonObject clip = entry.getValue().getAsJsonObject();
                boolean loop = clip.has("loop") && "true".equals(clip.get("loop").getAsString());
                double seconds = clip.has("animation_length") ? clip.get("animation_length").getAsDouble() : 1;
                clips.put(entry.getKey(), new Clip(entry.getKey(), Math.max(1, (int) Math.ceil(seconds * 20)), loop));
            });
            Clip idle = select(clips, "idle", "beast_idle", "core_idle", "shard_idle", "glide");
            if (idle == null || !idle.loop()) throw new IllegalStateException(path + " 缺少循环待机动作");
            return new Profile(path, Map.copyOf(clips), idle,
                select(clips, "walk", "beast_walk", "core_crawl", "shard_crawl", "flap", "fly"),
                select(clips, "run", "gallop"));
        } catch (IOException error) {
            throw new IllegalStateException("读取生物动画失败：" + resource, error);
        }
    }

    private static Clip select(Map<String, Clip> clips, String... candidates) {
        for (String candidate : candidates) {
            for (Clip clip : clips.values()) {
                if (shortName(clip.name()).equals(candidate) && clip.loop()) return clip;
            }
        }
        return null;
    }

    /** 形态切换必须同时切换几何、纹理和动画，不能把不同绑定姿的骨轨道混用。 */
    public static List<Profile> profiles(FaunaVisualKind kind) {
        return switch (kind) {
            case HYBRID_BEAST -> List.of(load("hybrid_beast"), load("hybrid_beast_core"), load("hybrid_beast_shard"));
            case FUYU_VULTURE -> List.of(load("fuyu_vulture"), load("fuyu_vulture_flight"));
            default -> List.of(kind.animations());
        };
    }
}
