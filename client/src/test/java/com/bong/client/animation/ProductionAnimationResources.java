package com.bong.client.animation;

import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationRegistry;
import net.minecraft.resource.InputSupplier;
import net.minecraft.resource.Resource;
import net.minecraft.resource.ResourceManager;
import net.minecraft.resource.ResourcePack;
import net.minecraft.util.Identifier;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.function.Predicate;
import java.util.stream.Stream;

/**
 * plan-skill-av-relink-v1 P3 —— 用<b>生产 resource reload 入口</b>装载真实
 * main-resources 动画资产的测试工具。
 *
 * <p>PlayerAnimator 的 Fabric reload listener 最终调
 * {@link PlayerAnimationRegistry#resourceLoaderCallback}（清空注册表 →
 * {@code findResources("player_animation", ...)} → 逐文件
 * {@code AnimationSerializing} 反序列化 → 按 JSON {@code name} 字段注册）。
 * 本工具构造一个只暴露 {@code client/src/main/resources/assets/bong/player_animation}
 * 真实文件的 {@link ResourceManager}，调用<b>同一个</b>生产入口——headless 单测由此
 * 锁住「生产 reload 语义能把 shipped 资产装进 {@link BongAnimationRegistry} 的
 * JSON 源」这条链路，而非绕行 {@code registerInlineJson} 手工灌注。
 */
public final class ProductionAnimationResources {
    private ProductionAnimationResources() {
    }

    /**
     * 经生产 reload 入口装载全部 main-resources 动画资产（幂等：入口自身先清空
     * PlayerAnimator 注册表再全量重灌，与 F3+T 资源重载语义一致）。
     */
    public static void loadViaProductionReloadCallback() {
        PlayerAnimationRegistry.resourceLoaderCallback(
            mainResourcesAnimationManager(),
            LoggerFactory.getLogger("bong-test-anim-reload")
        );
    }

    /** 只暴露真实 main-resources {@code player_animation} 目录的 ResourceManager。 */
    static ResourceManager mainResourcesAnimationManager() {
        Path root = animationAssetRoot();
        return new ResourceManager() {
            @Override
            public Set<String> getAllNamespaces() {
                return Set.of(BongAnimations.MOD_ID);
            }

            @Override
            public Optional<Resource> getResource(Identifier id) {
                if (!BongAnimations.MOD_ID.equals(id.getNamespace())
                    || !id.getPath().startsWith("player_animation/")) {
                    return Optional.empty();
                }
                Path file = root.resolve(
                    id.getPath().substring("player_animation/".length()));
                return Files.isRegularFile(file)
                    ? Optional.of(new Resource(null, InputSupplier.create(file)))
                    : Optional.empty();
            }

            @Override
            public List<Resource> getAllResources(Identifier id) {
                return getResource(id).map(List::of).orElse(List.of());
            }

            @Override
            public Map<Identifier, Resource> findResources(
                String startingPath,
                Predicate<Identifier> allowedPathPredicate
            ) {
                if (!"player_animation".equals(startingPath)) {
                    return Map.of();
                }
                Map<Identifier, Resource> found = new LinkedHashMap<>();
                try (Stream<Path> files = Files.list(root)) {
                    files.filter(Files::isRegularFile).sorted().forEach(file -> {
                        Identifier id = new Identifier(
                            BongAnimations.MOD_ID,
                            startingPath + "/" + file.getFileName());
                        if (allowedPathPredicate.test(id)) {
                            found.put(id, new Resource(null, InputSupplier.create(file)));
                        }
                    });
                } catch (IOException exception) {
                    throw new UncheckedIOException(
                        "枚举 main-resources 动画目录失败：" + root, exception);
                }
                return found;
            }

            @Override
            public Map<Identifier, List<Resource>> findAllResources(
                String startingPath,
                Predicate<Identifier> allowedPathPredicate
            ) {
                Map<Identifier, List<Resource>> found = new LinkedHashMap<>();
                findResources(startingPath, allowedPathPredicate)
                    .forEach((id, resource) -> found.put(id, List.of(resource)));
                return found;
            }

            @Override
            public Stream<ResourcePack> streamResourcePacks() {
                return Stream.empty();
            }
        };
    }

    /**
     * 生产动画资产根（与 {@code AnimWiringManifestTest#animationAssetRoot()} 同语义：
     * 绑定 main source set，定位失败抛错不降级）。
     */
    static Path animationAssetRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/main/resources/assets/bong/player_animation"),
                cwd.resolve("client/src/main/resources/assets/bong/player_animation"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException(
            "无法定位 client main resources 的 player_animation 根（user.dir=" + cwd
                + "）——生产 reload 测试必须绑定 main source set");
    }
}
