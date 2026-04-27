package com.bong.client.combat;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-armor-v1 §5 review feedback —— client {@link ArmorProfileStore}
 * 是 server JSON 蓝图的硬编码镜像。drift 一旦发生 server JSON 改了 client 不会
 * 报错，这条测试把每个 server profile 的 kind_mitigation 与 client 常量
 * 显式 cross-check，drift 时 fail-loud。
 *
 * <p>当 §3 ArmorDurabilityChanged + 完整 profile 网络下发就绪后可删。
 */
class ArmorProfileStoreCrossCheckTest {

    /** Resolved at runtime: gradle test 默认 cwd = client/, server assets 在 ../server/. */
    private static final Path SERVER_ARMOR_PROFILES = Path.of("..", "server", "assets", "combat", "armor_profiles");

    private static final float TOLERANCE = 1e-4f;

    @Test
    void clientMirrorMatchesEveryServerProfile() throws IOException {
        assertTrue(Files.isDirectory(SERVER_ARMOR_PROFILES),
            "server armor profile dir not found at " + SERVER_ARMOR_PROFILES.toAbsolutePath());

        try (Stream<Path> files = Files.list(SERVER_ARMOR_PROFILES)) {
            files
                .filter(p -> p.getFileName().toString().endsWith(".json"))
                .forEach(this::assertMatchesClient);
        }
    }

    private void assertMatchesClient(Path jsonFile) {
        JsonObject root;
        try {
            root = JsonParser.parseString(Files.readString(jsonFile)).getAsJsonObject();
        } catch (IOException e) {
            throw new AssertionError("Failed to read " + jsonFile, e);
        }

        String templateId = root.get("template_id").getAsString();
        JsonObject mitigation = root.getAsJsonObject("profile").getAsJsonObject("kind_mitigation");

        ArmorProfileStore.ArmorMitigation client = ArmorProfileStore.mitigationForItemId(templateId);
        assertNotNull(client,
            "ArmorProfileStore mirror is missing template_id '" + templateId + "' from " + jsonFile.getFileName()
                + " — add it to BY_ITEM_ID or remove the server JSON.");

        assertEquals(mitigation.get("cut").getAsFloat(), client.cut(), TOLERANCE,
            "cut mismatch for " + templateId);
        assertEquals(mitigation.get("blunt").getAsFloat(), client.blunt(), TOLERANCE,
            "blunt mismatch for " + templateId);
        assertEquals(mitigation.get("pierce").getAsFloat(), client.pierce(), TOLERANCE,
            "pierce mismatch for " + templateId);
        assertEquals(mitigation.get("burn").getAsFloat(), client.burn(), TOLERANCE,
            "burn mismatch for " + templateId);
        assertEquals(mitigation.get("concussion").getAsFloat(), client.concussion(), TOLERANCE,
            "concussion mismatch for " + templateId);
    }
}
