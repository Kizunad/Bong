package com.bong.client.visual.particle;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 粒子资产三方对账 pin：particles/*.json 引用的每张贴图，必须
 * ① 在 atlases/particles.json 白名单里登记（single 条目），且
 * ② textures/particle/ 下真实存在对应 PNG。
 *
 * 背景：MC 1.20.1 的粒子图集按 atlases/particles.json 白名单缝合——只有
 * 登记过的贴图才进图集，漏登记 = sprite 缺失 = 粒子隐形，且只有一条 30s
 * 节流的 "Missing particle sprites" WARN。vortex_spiral（涡流全系 15 招
 * 共用）与 dugu 三张贴图自 PR #173 起漏登记，导致涡流粒子上线以来全部
 * 隐形、参数怎么调都"没作用"。本测试让"加了贴图/JSON 却忘登记图集"这类
 * 静默孤岛永远在 CI 撞红。
 */
class ParticleAtlasReconciliationTest {

    private static final Path RESOURCES = Path.of("src", "main", "resources");
    private static final Path PARTICLES_DIR = RESOURCES.resolve(Path.of("assets", "bong", "particles"));
    private static final Path TEXTURES_DIR =
        RESOURCES.resolve(Path.of("assets", "bong", "textures", "particle"));
    private static final Path ATLAS_JSON =
        RESOURCES.resolve(Path.of("assets", "minecraft", "atlases", "particles.json"));

    @Test
    void everyReferencedParticleTextureIsWhitelistedInAtlasAndHasPng() throws IOException {
        Set<String> atlasEntries = atlasSingleResources();
        List<String> problems = new java.util.ArrayList<>();

        try (Stream<Path> jsons = Files.list(PARTICLES_DIR)) {
            for (Path json : jsons.filter(p -> p.toString().endsWith(".json")).toList()) {
                for (String textureId : referencedTextures(json)) {
                    if (!textureId.startsWith("bong:particle/")) {
                        problems.add(json.getFileName() + " 引用了非常规贴图 id `" + textureId
                            + "`（本仓约定 bong:particle/<name>）");
                        continue;
                    }
                    String name = textureId.substring("bong:particle/".length());
                    if (!atlasEntries.contains(textureId)) {
                        problems.add(json.getFileName() + " 的贴图 `" + textureId
                            + "` 未在 atlases/particles.json 登记 —— 图集不会缝合它，"
                            + "粒子将静默隐形（vortex_spiral 前车之鉴）");
                    }
                    Path png = TEXTURES_DIR.resolve(name + ".png");
                    if (!Files.exists(png)) {
                        problems.add(json.getFileName() + " 的贴图 `" + textureId
                            + "` 缺少 PNG 文件 " + png);
                    }
                }
            }
        }

        assertTrue(problems.isEmpty(),
            "粒子资产对账失败（" + problems.size() + " 项）：\n  " + String.join("\n  ", problems));
    }

    @Test
    void atlasWhitelistHasNoDanglingEntries() throws IOException {
        // 反向对账：白名单里每条 bong 贴图都应有 PNG——防改名后旧条目烂在清单里
        // 造成"看似登记过"的错觉。
        List<String> problems = new java.util.ArrayList<>();
        for (String resource : atlasSingleResources()) {
            if (!resource.startsWith("bong:particle/")) continue;
            String name = resource.substring("bong:particle/".length());
            if (!Files.exists(TEXTURES_DIR.resolve(name + ".png"))) {
                problems.add("atlases/particles.json 登记的 `" + resource + "` 无对应 PNG");
            }
        }
        assertTrue(problems.isEmpty(), String.join("\n", problems));
    }

    private static Set<String> atlasSingleResources() throws IOException {
        JsonObject root = JsonParser.parseReader(
            new InputStreamReader(Files.newInputStream(ATLAS_JSON), StandardCharsets.UTF_8))
            .getAsJsonObject();
        Set<String> out = new HashSet<>();
        for (JsonElement source : root.getAsJsonArray("sources")) {
            JsonObject obj = source.getAsJsonObject();
            if ("single".equals(obj.get("type").getAsString())) {
                out.add(obj.get("resource").getAsString());
            }
        }
        return out;
    }

    private static List<String> referencedTextures(Path particleJson) throws IOException {
        JsonObject root = JsonParser.parseReader(
            new InputStreamReader(Files.newInputStream(particleJson), StandardCharsets.UTF_8))
            .getAsJsonObject();
        JsonArray textures = root.getAsJsonArray("textures");
        List<String> out = new java.util.ArrayList<>();
        if (textures != null) {
            for (JsonElement t : textures) out.add(t.getAsString());
        }
        return out;
    }
}
