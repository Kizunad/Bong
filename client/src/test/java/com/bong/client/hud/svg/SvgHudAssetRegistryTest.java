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
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SvgHudAssetRegistryTest {
    @Test
    void requiresAResourceManagerBeforeLoadingAssets() {
        assertThrows(NullPointerException.class, () -> new SvgHudAssetRegistry(null));
    }

    @Test
    void loadsAndCachesTheShippedQiRadarFromTheProductionResourcePath() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());

        SvgMesh first = registry.get("bong-client:svg/hud/qi-radar.svg");
        SvgMesh second = registry.get(new Identifier("bong-client", "svg/hud/qi-radar.svg"));

        assertTrue(first.triangleCount() > 0, "真实 QI_RADAR SVG 必须生成非空 mesh");
        assertEquals(60, first.triangleCount(), "QI_RADAR 资产的图元 tessellation 数量漂移");
        assertSame(first, second, "重复读取同一资源必须命中 registry 的 immutable mesh 缓存");
    }

    @Test
    void loadsTheHighContrastPreviewExample() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());

        SvgMesh example = registry.get("bong-client:svg/hud/example.svg");

        assertTrue(example.triangleCount() > 0, "SVG 预览示例必须生成非空 mesh");
    }

    @Test
    void rejectsResourcesOutsideTheHudSvgAllowlist() {
        SvgHudAssetRegistry registry = new SvgHudAssetRegistry(mainResourceManager());

        assertThrows(IllegalArgumentException.class, () -> registry.get("bong-client:textures/gui/not-svg.png"));
        assertThrows(IllegalArgumentException.class, () -> registry.get("bong-client:svg/hud/../other.svg"));
        assertThrows(IllegalArgumentException.class, () -> registry.get("other:svg/hud/qi-radar.svg"));
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
}
