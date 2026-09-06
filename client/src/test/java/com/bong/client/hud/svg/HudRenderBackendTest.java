package com.bong.client.hud.svg;

import com.bong.client.hud.ScreenHudVisibility;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** SVG 基础设施契约：后端边界与资源重载不依赖具体 HUD 组件。 */
class HudRenderBackendTest {
    private static final Path CLIENT_ROOT = Path.of("").toAbsolutePath().normalize();

    @Test
    void contractFixtureNamesExistingBackendTypesAndTests() throws IOException {
        List<String> rows = fixture("/bong/ui/ui-svg-hud-contract.tsv");
        assertEquals(8, rows.size(), "SVG contract 必须登记后端接口、解析边界和八个基础类型");
        for (String row : rows) {
            String[] fields = row.split("\\t", -1);
            assertEquals(5, fields.length, "SVG contract 行字段数错误: " + row);
            Path owner = sourcePath(fields[1]);
            assertTrue(Files.isDirectory(owner), "SVG owner 目录不存在: " + owner);
            assertTrue(Files.exists(owner.resolve(fields[0] + ".java")),
                "SVG contract 类型不存在: " + owner.resolve(fields[0] + ".java"));
            assertEquals("IMPLEMENTED", fields[3], "已登记的 SVG 基础设施必须有实现状态");
            assertTrue(testClassExists(fields[4]), "SVG contract test owner 不存在: " + fields[4]);
        }
    }

    @Test
    void svgBackendUsesTheFrozenVisibilityAndGuiSubmissionBoundary() throws IOException {
        String backend = Files.readString(sourcePath("src/main/java/com/bong/client/hud/svg/SvgHudBackend.java"));
        String bongHud = Files.readString(sourcePath("src/main/java/com/bong/client/BongHud.java"));
        String bongClient = Files.readString(sourcePath("src/main/java/com/bong/client/BongClient.java"));
        assertTrue(backend.contains("implements HudRenderBackend"));
        assertTrue(backend.contains("visibility != ScreenHudVisibility.FULL"),
            "SVG layer 必须遵守 FULL HUD 可见性门");
        assertTrue(bongHud.contains("HudRenderBackend backend"),
            "BongHud 必须只依赖表现后端接口");
        assertTrue(bongHud.contains("backend.render(context, client, visibility)"),
            "生产 HUD 必须把 ScreenHudVisibility 交给注入的后端");
        assertFalse(bongHud.contains("SvgHudBackend"),
            "BongHud 不得直接依赖具体 SVG 后端");
        assertTrue(bongClient.contains("new BongHudRenderer(SvgHudBackend.production())"),
            "SVG 具体实现必须只在 BongClient 组合根装配");
        assertFalse(backend.contains("PlayerStateStore"),
            "SVG 后端不得直接读取 PlayerStateStore");
        assertFalse(backend.contains("PlayerStateViewModel"),
            "SVG 后端不得直接依赖 PlayerStateViewModel");
        assertFalse(backend.contains("HudRealmGate"),
            "SVG 后端不得直接执行境界门控");
        assertTrue(backend.contains("MinecraftGuiMeshEmitter"));
        assertFalse(backend.contains("RenderSystem"), "SVG 后端不得直接触碰 OpenGL 提交 API");
    }

    @Test
    void reloadListenerInvalidatesTheCachedBackendRegistry() {
        SvgHudBackend.resetForTests();
        ResourceManager manager = emptyResourceManager();
        SvgHudAssetRegistry beforeReload = SvgHudBackend.registry(manager);

        new SvgHudResourceReloadListener().reload(manager);

        SvgHudAssetRegistry afterReload = SvgHudBackend.registry(manager);
        assertFalse(beforeReload == afterReload,
            "资源重载必须丢弃旧 registry，避免 F3+T 后继续使用旧资源包的 mesh");
        SvgHudBackend.resetForTests();
    }

    private static List<String> fixture(String resource) throws IOException {
        try (var input = HudRenderBackendTest.class.getResourceAsStream(resource)) {
            assertNotNull(input, "缺少 SVG fixture: " + resource);
            return new String(input.readAllBytes(), StandardCharsets.UTF_8).lines()
                .filter(line -> !line.isBlank() && !line.stripLeading().startsWith("#"))
                .toList();
        }
    }

    private static boolean testClassExists(String simpleName) {
        return Files.exists(sourcePath("src/test/java/com/bong/client/hud/svg/" + simpleName + ".java"))
            || Files.exists(sourcePath("src/test/java/com/bong/client/hud/" + simpleName + ".java"));
    }

    private static Path sourcePath(String relative) {
        Path base = Files.isDirectory(CLIENT_ROOT.resolve("src")) ? CLIENT_ROOT : CLIENT_ROOT.resolve("client");
        String normalized = relative.startsWith("client/") ? relative.substring("client/".length()) : relative;
        return base.resolve(normalized);
    }

    private static ResourceManager emptyResourceManager() {
        return new ResourceManager() {
            @Override
            public java.util.Set<String> getAllNamespaces() {
                return java.util.Set.of("bong-client");
            }

            @Override
            public java.util.Optional<net.minecraft.resource.Resource> getResource(Identifier id) {
                return java.util.Optional.empty();
            }

            @Override
            public List<net.minecraft.resource.Resource> getAllResources(Identifier id) {
                return List.of();
            }

            @Override
            public java.util.Map<Identifier, net.minecraft.resource.Resource> findResources(
                String startingPath,
                java.util.function.Predicate<Identifier> allowedPathPredicate
            ) {
                return java.util.Map.of();
            }

            @Override
            public java.util.Map<Identifier, List<net.minecraft.resource.Resource>> findAllResources(
                String startingPath,
                java.util.function.Predicate<Identifier> allowedPathPredicate
            ) {
                return java.util.Map.of();
            }

            @Override
            public java.util.stream.Stream<net.minecraft.resource.ResourcePack> streamResourcePacks() {
                return java.util.stream.Stream.empty();
            }
        };
    }
}
