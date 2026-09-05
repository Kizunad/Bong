package com.bong.client.hud.svg;

import net.minecraft.resource.InputSupplier;
import net.minecraft.resource.Resource;
import net.minecraft.resource.ResourceManager;
import net.minecraft.resource.ResourcePack;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.function.Predicate;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SvgHudAssetRegistryTest {
    @Test
    void registeredProductionAssetsHaveReusableGeometryAndViewport() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());
        for (var surface : com.bong.client.hud.HudRenderRegistry.productionSurfaces()) {
            for (var asset : surface.svgAssets()) {
                SvgMesh mesh = registry.get(asset.resource());
                assertTrue(mesh.triangleCount() > 0, "已登记的生产资产必须有几何: " + asset.resource());
                assertTrue(mesh.width() > 0 && mesh.height() > 0, "动态缩放需要保留 SVG 画布尺寸");
                assertSame(mesh, registry.get(asset.resource()), "帧间必须重用资源几何");
                registry.clear();
                org.junit.jupiter.api.Assertions.assertNotSame(mesh, registry.get(asset.resource()),
                    "资源重载后必须重新读取并三角化，不能保留旧资源包几何");
            }
        }
        assertThrows(IllegalArgumentException.class, () -> com.bong.client.hud.HudRenderRegistry
            .requireVectorAsset(com.bong.client.hud.HudRenderLayer.QUICK_BAR, "body"),
            "不能跨 layer 使用未登记的图形 key");
    }

    @Test
    void requiresAResourceManagerBeforeLoadingAssets() {
        assertThrows(NullPointerException.class, () -> new SvgHudAssetRegistry(null));
    }

    @Test
    void loadsAndCachesThePreviewExampleFromTheShippedResourcePath() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());

        SvgMesh first = registry.get("bong-client:svg/hud/example.svg");
        SvgMesh second = registry.get(Identifier.of("bong-client", "svg/hud/example.svg"));

        assertTrue(first.triangleCount() > 0, "SVG 预览示例必须生成非空 mesh");
        assertSame(first, second, "重复读取同一资源必须命中 registry 的 immutable mesh 缓存");
    }

    @Test
    void rejectsResourcesOutsideTheHudSvgAllowlist() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());

        assertThrows(IllegalArgumentException.class, () -> registry.get("bong-client:textures/gui/not-svg.png"));
        assertThrows(IllegalArgumentException.class, () -> registry.get("bong-client:svg/hud/../other.svg"));
        assertThrows(IllegalArgumentException.class, () -> registry.get("other:svg/hud/example.svg"));
    }

    @Test
    void cachesMissingResourceFailureUntilExplicitClear() {
        int[] lookups = {0};
        ResourceManager manager = missingResourceManager(lookups);
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(manager);
        Identifier missing = Identifier.of("bong-client", "svg/hud/missing.svg");

        assertFalse(registry.find(missing).isPresent(), "缺失资源的首次加载必须 fail closed");
        assertFalse(registry.find(missing).isPresent(), "失败结果必须命中缓存，不能每帧重试读取");
        assertEquals(1, lookups[0], "同一失败资源在 clear 前只能向 ResourceManager 查询一次");

        registry.clear();
        assertFalse(registry.find(missing).isPresent(), "clear 后仍应保持 fail closed");
        assertEquals(2, lookups[0], "资源重载清 cache 后才允许重新尝试加载");
    }

    private static ResourceManager mainResourceManager() {
        Path root = Path.of("src/main/resources/assets/bong-client").toAbsolutePath().normalize();
        return new ResourceManager() {
            @Override
            public Set<String> getAllNamespaces() {
                return Set.of("bong-client");
            }

            @Override
            public Optional<Resource> getResource(Identifier id) {
                if (!"bong-client".equals(id.getNamespace()) || !id.getPath().startsWith("svg/hud/")) {
                    return Optional.empty();
                }
                Path file = root.resolve(id.getPath());
                return Files.isRegularFile(file)
                    ? Optional.of(new Resource(null, InputSupplier.create(file)))
                    : Optional.empty();
            }

            @Override
            public List<Resource> getAllResources(Identifier id) {
                return getResource(id).map(List::of).orElse(List.of());
            }

            @Override
            public Map<Identifier, Resource> findResources(String startingPath, Predicate<Identifier> allowedPathPredicate) {
                return Map.of();
            }

            @Override
            public Map<Identifier, List<Resource>> findAllResources(
                String startingPath,
                Predicate<Identifier> allowedPathPredicate
            ) {
                return Map.of();
            }

            @Override
            public Stream<ResourcePack> streamResourcePacks() {
                return Stream.empty();
            }
        };
    }

    private static ResourceManager missingResourceManager(int[] lookups) {
        return new ResourceManager() {
            @Override
            public Set<String> getAllNamespaces() {
                return Set.of("bong-client");
            }

            @Override
            public Optional<Resource> getResource(Identifier id) {
                lookups[0]++;
                return Optional.empty();
            }

            @Override
            public List<Resource> getAllResources(Identifier id) {
                return List.of();
            }

            @Override
            public Map<Identifier, Resource> findResources(String startingPath, Predicate<Identifier> allowedPathPredicate) {
                return Map.of();
            }

            @Override
            public Map<Identifier, List<Resource>> findAllResources(
                String startingPath,
                Predicate<Identifier> allowedPathPredicate
            ) {
                return Map.of();
            }

            @Override
            public Stream<ResourcePack> streamResourcePacks() {
                return Stream.empty();
            }
        };
    }
}
