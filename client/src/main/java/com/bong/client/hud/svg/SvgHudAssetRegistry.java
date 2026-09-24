package com.bong.client.hud.svg;

import net.minecraft.resource.Resource;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

/** HUD SVG 资源白名单与 immutable mesh 缓存。 */
public final class SvgHudAssetRegistry {
    private static final String ALLOWED_PREFIX = "svg/hud/";
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private final ResourceManager resourceManager;
    private final SvgParser parser;
    private final SvgTessellator tessellator;
    private final Map<Identifier, AssetLoadResult> cache = new ConcurrentHashMap<>();

    public SvgHudAssetRegistry(ResourceManager resourceManager) {
        this(resourceManager, new NanoSvgParser(), new SvgTessellator());
    }

    SvgHudAssetRegistry(ResourceManager resourceManager, SvgParser parser, SvgTessellator tessellator) {
        this.resourceManager = Objects.requireNonNull(resourceManager, "resourceManager");
        this.parser = Objects.requireNonNull(parser, "parser");
        this.tessellator = Objects.requireNonNull(tessellator, "tessellator");
    }

    public SvgMesh get(String resourceId) {
        Identifier identifier = Identifier.tryParse(resourceId);
        if (identifier == null) {
            throw new IllegalArgumentException("无效 SVG 资源 id: " + resourceId);
        }
        return get(identifier);
    }

    public SvgMesh get(Identifier identifier) {
        validate(identifier);
        return cached(identifier).requireMesh();
    }

    /**
     * 返回已成功加载的 mesh；资源加载失败也会写入缓存，后续渲染帧直接跳过。
     */
    Optional<SvgMesh> find(Identifier identifier) {
        validate(identifier);
        return cached(identifier).mesh();
    }

    public void clear() {
        cache.clear();
    }

    private AssetLoadResult cached(Identifier identifier) {
        return cache.computeIfAbsent(identifier, this::load);
    }

    private AssetLoadResult load(Identifier identifier) {
        try {
            Resource resource = resourceManager.getResource(identifier)
                .orElseThrow(() -> new IllegalArgumentException("SVG 资源不存在: " + identifier));
            try (var input = resource.getInputStream()) {
                SvgMesh mesh = tessellator.tessellate(parser.parse(input));
                LOGGER.info("[svg] 已加载 HUD 资源 {}，三角形数={}", identifier, mesh.triangleCount());
                return AssetLoadResult.success(mesh);
            }
        } catch (IOException | RuntimeException failure) {
            IllegalArgumentException cachedFailure = new IllegalArgumentException(
                "SVG 资源加载失败: " + identifier,
                failure
            );
            // 失败结果和成功 mesh 一样缓存，避免坏资源在每帧重复 I/O、解析和告警。
            LOGGER.error("[svg] HUD 资源加载失败，已缓存失败状态: {}", identifier, cachedFailure);
            return AssetLoadResult.failure(cachedFailure);
        }
    }

    private record AssetLoadResult(SvgMesh loadedMesh, IllegalArgumentException failure) {
        private AssetLoadResult {
            if ((loadedMesh == null) == (failure == null)) {
                throw new IllegalArgumentException("SVG 资源缓存结果必须且只能包含 mesh 或失败原因");
            }
        }

        static AssetLoadResult success(SvgMesh mesh) {
            return new AssetLoadResult(Objects.requireNonNull(mesh, "mesh"), null);
        }

        static AssetLoadResult failure(IllegalArgumentException failure) {
            return new AssetLoadResult(null, Objects.requireNonNull(failure, "failure"));
        }

        Optional<SvgMesh> mesh() {
            return Optional.ofNullable(loadedMesh);
        }

        SvgMesh requireMesh() {
            if (failure != null) {
                throw failure;
            }
            return loadedMesh;
        }
    }

    private static void validate(Identifier identifier) {
        Objects.requireNonNull(identifier, "identifier");
        String path = identifier.getPath();
        if (!"bong-client".equals(identifier.getNamespace())
            || !path.startsWith(ALLOWED_PREFIX)
            || !path.endsWith(".svg")
            || path.contains("..")) {
            throw new IllegalArgumentException("SVG 资源不在 HUD 白名单: " + identifier);
        }
    }
}
