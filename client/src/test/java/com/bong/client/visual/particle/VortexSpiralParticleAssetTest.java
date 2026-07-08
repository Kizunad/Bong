package com.bong.client.visual.particle;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

/**
 * r9 P1 #2：涡流招式共用粒子贴图链路 pin。
 *
 * <p>粒子 descriptor 与 PNG 存在还不够；MC 1.20.1 只会缝合
 * atlases/particles.json 中显式列出的 single source。缺少该 atlas source 时，
 * VortexSpiralPlayer 能正常 spawn 粒子，但 sprite 为空，涡流 VFX 会静默隐形。
 */
class VortexSpiralParticleAssetTest {

    private static final String TEXTURE_ID = "bong:particle/vortex_spiral";
    private static final Path PARTICLE_DESCRIPTOR =
        Path.of("src/main/resources/assets/bong/particles/vortex_spiral.json");
    private static final Path PARTICLE_TEXTURE =
        Path.of("src/main/resources/assets/bong/textures/particle/vortex_spiral.png");
    private static final Path PARTICLE_ATLAS =
        Path.of("src/main/resources/assets/minecraft/atlases/particles.json");

    @Test
    void vortexSpiralDescriptorReferencesCommittedTexture() throws IOException {
        assertTrue(Files.isRegularFile(PARTICLE_DESCRIPTOR),
            "涡流粒子 descriptor 不存在: " + PARTICLE_DESCRIPTOR);
        assertTrue(Files.isRegularFile(PARTICLE_TEXTURE),
            "涡流粒子 PNG 不存在: " + PARTICLE_TEXTURE);

        JsonObject root = parseJson(PARTICLE_DESCRIPTOR);
        assertTrue(root.has("textures"), "vortex_spiral.json 必须声明 textures 数组");
        assertEquals(TEXTURE_ID, root.getAsJsonArray("textures").get(0).getAsString(),
            "vortex_spiral.json 必须引用 " + TEXTURE_ID);
    }

    @Test
    void vortexSpiralTextureIsRegisteredAsParticleAtlasSingleSource() throws IOException {
        JsonObject root = parseJson(PARTICLE_ATLAS);
        for (JsonElement source : root.getAsJsonArray("sources")) {
            JsonObject sourceObj = source.getAsJsonObject();
            if ("single".equals(sourceObj.get("type").getAsString())
                && TEXTURE_ID.equals(sourceObj.get("resource").getAsString())) {
                return;
            }
        }

        fail(TEXTURE_ID + " 必须在 minecraft/atlases/particles.json 中注册为 single source，"
            + "否则 MC 不会把涡流贴图缝合进粒子图集，涡流 VFX 会空白");
    }

    private static JsonObject parseJson(Path path) throws IOException {
        return JsonParser.parseString(Files.readString(path, StandardCharsets.UTF_8)).getAsJsonObject();
    }
}
