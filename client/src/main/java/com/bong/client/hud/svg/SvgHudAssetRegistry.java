package com.bong.client.hud.svg;

import net.minecraft.resource.Resource;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

/** HUD SVG 资源白名单与 immutable mesh 缓存。 */
public final class SvgHudAssetRegistry {
    private static final String ALLOWED_PREFIX = "svg/hud/";
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-svg-hud");
    private final ResourceManager resourceManager;
    private final SvgParser parser;
    private final SvgTessellator tessellator;
    private final Map<Identifier, SvgMesh> cache = new ConcurrentHashMap<>();

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
        return cache.computeIfAbsent(identifier, this::load);
    }

    public void clear() {
        cache.clear();
    }

    private SvgMesh load(Identifier identifier) {
        try {
            Resource resource = resourceManager.getResource(identifier)
                .orElseThrow(() -> new IllegalArgumentException("SVG 资源不存在: " + identifier));
            try (var input = resource.getInputStream()) {
                SvgMesh mesh = tessellator.tessellate(parser.parse(input));
                LOGGER.info("[svg] 已加载 HUD 资源 {}，三角形数={}", identifier, mesh.triangleCount());
                return mesh;
            }
        } catch (IOException e) {
            throw new IllegalArgumentException("SVG 资源读取失败: " + identifier, e);
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
