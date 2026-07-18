package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-av-relink-v1 P3 —— P1 接线 anim_id 共享清单的 client 侧消费闭环（动画链）。
 *
 * <p>消费与 server 侧 {@code anim_wiring_manifest_test.rs} 同一份 checked-in 清单
 * {@code bong/anim_wiring_manifest.json}（server {@code P1_WIRED_ANIM_IDS} 常量表的
 * 单向导出，重生成唯一入口
 * {@code cd server && BONG_REGEN_ANIM_MANIFEST=1 cargo test anim_wiring_manifest}），
 * 逐条断言 server 会发射的每个 anim_id 在 client 端真的能播出来：
 * <ol>
 *   <li>id 形态合法（{@code bong} 命名空间 + {@link Identifier} 可解析）且在
 *       {@link BongAnimations} 常量表有对应声明——server 发射的 id 不许绕过 client 常量锚；</li>
 *   <li>{@code player_animation/<id>.json} 资产在 <b>main resources</b>（生产资源包源）
 *       真实存在，且 JSON {@code name} 字段与文件名一致——PlayerAnimator 的
 *       {@code resourceLoaderCallback} 按 JSON {@code name}（非文件名）为运行时注册表键，
 *       两者不一致时 {@code bong:<id>} 在真机上永远解析不到；</li>
 *   <li>资产内容能被生产反序列化器（{@code AnimationSerializing}，与资源 reload 同一条路径）
 *       解析为可播放的 KeyframeAnimation，并经 {@link BongAnimationRegistry#contains} /
 *       {@link BongAnimationRegistry#get} 命中。</li>
 * </ol>
 */
public class AnimWiringManifestTest {
    static final String MANIFEST_RESOURCE = "bong/anim_wiring_manifest.json";

    private static final String REGEN_HINT =
        "清单由 server P1_WIRED_ANIM_IDS 单向生成、禁止手改；重生成："
            + "cd server && BONG_REGEN_ANIM_MANIFEST=1 cargo test anim_wiring_manifest";

    @AfterEach
    void tearDown() {
        BongAnimationRegistry.clearInlineForTest();
    }

    /** 供本类与 {@code VfxEventRouterTest} 共用的清单读取入口（classloader → 有序 id 列表）。 */
    public static List<String> loadManifest() throws IOException {
        try (InputStream input = AnimWiringManifestTest.class.getClassLoader()
                .getResourceAsStream(MANIFEST_RESOURCE)) {
            assertNotNull(input, "缺少 anim 接线清单 test fixture: " + MANIFEST_RESOURCE
                + " —— " + REGEN_HINT);
            JsonArray root = JsonParser
                .parseReader(new InputStreamReader(input, StandardCharsets.UTF_8))
                .getAsJsonArray();
            List<String> ids = new ArrayList<>();
            for (JsonElement element : root) {
                ids.add(element.getAsString());
            }
            assertFalse(ids.isEmpty(), "anim 接线清单不应为空数组 —— " + REGEN_HINT);
            return ids;
        }
    }

    /**
     * 生产动画资产根 {@code client/src/main/resources/assets/bong/player_animation}
     * （gradle test 工作目录 = client/；兼容从仓库根启动）。定位失败直接抛错——
     * 存在性检查绝不静默降级回聚合 classpath（防 test resources 假资产冒充）。
     */
    private static Path animationAssetRoot() {
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
                + "）——资产存在性检查必须绑定 main source set");
    }

    /** {@link BongAnimations} 声明的全部公开 {@link Identifier} 常量（client 侧 id 锚集合）。 */
    private static Set<String> declaredBongAnimationConstants() throws IllegalAccessException {
        Set<String> declared = new HashSet<>();
        for (Field field : BongAnimations.class.getFields()) {
            if (field.getType() == Identifier.class && Modifier.isStatic(field.getModifiers())) {
                declared.add(field.get(null).toString());
            }
        }
        assertFalse(declared.isEmpty(), "BongAnimations 反射不到任何 Identifier 常量——反射前提坏了");
        return declared;
    }

    /** 清单形态：非空 + 严格升序（即已排序且无重复），锁 server 侧 BTreeSet 导出契约。 */
    @Test
    void manifestIsSortedAndUnique() throws IOException {
        List<String> ids = loadManifest();
        for (int i = 1; i < ids.size(); i++) {
            assertTrue(ids.get(i - 1).compareTo(ids.get(i)) < 0,
                "清单第 " + i + " 项 `" + ids.get(i) + "` 未严格大于前项 `" + ids.get(i - 1)
                    + "`（排序漂移或重复条目，疑似手改）—— " + REGEN_HINT);
        }
    }

    /** 每条 id 都是 {@code bong} 命名空间、{@link Identifier} 可解析的合法 anim id。 */
    @Test
    void everyManifestEntryIsWellFormedBongIdentifier() throws IOException {
        for (String raw : loadManifest()) {
            Identifier id = Identifier.tryParse(raw);
            assertNotNull(id, "清单条目 `" + raw + "` 无法被 Identifier.tryParse 解析——"
                + "client 路由端会静默丢弃该 anim id");
            assertEquals(BongAnimations.MOD_ID, id.getNamespace(),
                "清单条目 `" + raw + "` 命名空间必须是 `" + BongAnimations.MOD_ID
                    + "`——player_animation 资产目录与 BongAnimations 常量表只认该命名空间");
        }
    }

    /** server 发射的每条 anim id 在 client {@link BongAnimations} 常量表都有对应声明。 */
    @Test
    void everyManifestEntryHasBongAnimationsConstant() throws Exception {
        Set<String> declared = declaredBongAnimationConstants();
        List<String> missing = new ArrayList<>();
        for (String raw : loadManifest()) {
            if (!declared.contains(raw)) {
                missing.add(raw);
            }
        }
        assertTrue(missing.isEmpty(),
            "以下 server P1 接线 anim id 在 client BongAnimations 常量表无对应 Identifier 声明："
                + missing + "——server 发射的 id 必须有 client 侧常量锚（防双端各写一份字面量漂移）");
    }

    /**
     * 每条接线动画的 {@code player_animation/<id>.json} 在生产资源包源真实存在，
     * 且是合法 Emotecraft v3 运行时 JSON：{@code name} == 文件名（PlayerAnimator 运行时
     * 注册表按 JSON name 为键，不一致 = 真机上 id 永远解析不到）、endTick 为正、
     * 弧度制、含关键帧。
     */
    @Test
    void everyWiredAnimHasValidPlayerAnimationAsset() throws IOException {
        Path root = animationAssetRoot();
        for (String raw : loadManifest()) {
            String path = raw.substring(raw.indexOf(':') + 1);
            Path asset = root.resolve(path + ".json");
            assertTrue(Files.isRegularFile(asset),
                "接线动画 `" + raw + "` 缺少生产资产 " + asset
                    + "——server 会发射该 anim id，client 无资产可播（P1 接线断链）");

            JsonObject json = JsonParser.parseString(Files.readString(asset)).getAsJsonObject();
            assertEquals(3, json.get("version").getAsInt(),
                raw + " 资产必须是 Emotecraft v3 JSON");
            assertEquals(path, json.get("name").getAsString(),
                raw + " 资产 JSON name 必须与文件名一致——PlayerAnimator 运行时注册表按 name"
                    + "（非文件名）为键，不一致时 `" + raw + "` 在真机上永远查不到");
            JsonObject emote = json.getAsJsonObject("emote");
            assertTrue(emote.get("endTick").getAsInt() > 0, raw + " endTick 必须为正");
            assertFalse(emote.get("degrees").getAsBoolean(), raw + " 运行时 JSON 应使用弧度");
            assertTrue(emote.getAsJsonArray("moves").size() > 0, raw + " 必须含关键帧");
        }
    }

    /**
     * 生产 reload 闭环：经 {@link ProductionAnimationResources}（同一个
     * {@code PlayerAnimationRegistry.resourceLoaderCallback} 生产入口 + 真实
     * main-resources 文件）装载后，每条接线 anim id 在
     * {@link BongAnimationRegistry} 逐条命中且来源为 <b>JSON 源</b>——锁住
     * "生产 resource reload 语义（含按 JSON {@code name} 字段注册）真的能把
     * shipped 资产送进运行时注册表"，防 loader 目录/命名/reload 接线回归。
     */
    @Test
    void everyWiredAnimLoadsThroughProductionResourceReloadCallback() throws IOException {
        ProductionAnimationResources.loadViaProductionReloadCallback();
        for (String raw : loadManifest()) {
            Identifier id = Identifier.tryParse(raw);
            assertNotNull(id, "清单条目 `" + raw + "` 应可解析（见形态用例）");
            assertTrue(BongAnimationRegistry.contains(id),
                "生产 reload 入口装载后 BongAnimationRegistry.contains(" + raw
                    + ") 应为 true——resource reload 链路（目录扫描/name 键注册）断了，"
                    + "真机上该 anim id 永远解析不到");
            assertNotNull(BongAnimationRegistry.get(id),
                "生产 reload 装载后 BongAnimationRegistry.get(" + raw + ") 不应为 null");
            assertEquals(BongAnimationRegistry.Source.JSON, BongAnimationRegistry.sourceOf(id),
                raw + " 应命中 JSON 源（PlayerAnimator 资源注册表）——若落到其他源，"
                    + "说明生产 reload 没装进该资产、命中是测试污染的假阳性");
        }
    }

    /**
     * 参数化反序列化 pin：每条接线动画的资产内容经生产反序列化器
     * （{@code AnimationSerializing}，与资源 reload listener 同一实现）可读成
     * 可播放动画，{@link BongAnimationRegistry#contains} / {@link BongAnimationRegistry#get}
     * 命中 inline 源。生产 reload 全链路（目录扫描 → name 键注册）由上方
     * {@code everyWiredAnimLoadsThroughProductionResourceReloadCallback} 单独锁住，
     * 本用例只锁资产字节可解析性。
     */
    @Test
    void everyWiredAnimResolvesThroughBongAnimationRegistry() throws IOException {
        Path root = animationAssetRoot();
        for (String raw : loadManifest()) {
            Identifier id = Identifier.tryParse(raw);
            assertNotNull(id, "清单条目 `" + raw + "` 应可解析（见形态用例）");
            String assetJson = Files.readString(root.resolve(id.getPath() + ".json"));

            assertTrue(BongAnimationRegistry.registerInlineJson(id, assetJson),
                "接线动画 `" + raw + "` 的资产 JSON 无法被生产 AnimationSerializing 反序列化"
                    + "为可播放动画——真机资源 reload 走同一条解析路径，该资产等于坏档");
            assertTrue(BongAnimationRegistry.contains(id),
                "注册后 BongAnimationRegistry.contains(" + raw + ") 应为 true——"
                    + "contains 契约破了，路由层会把可播动画当未知 id 降级丢弃");
            assertNotNull(BongAnimationRegistry.get(id),
                "注册后 BongAnimationRegistry.get(" + raw + ") 不应为 null");
        }
    }

    /** 负向对照：未注册的探针 id 不被 contains 命中——证明 contains pin 不是恒真断言。 */
    @Test
    void unknownAnimIdIsNotContained() {
        Identifier probe = new Identifier(BongAnimations.MOD_ID, "__anim_wiring_probe_missing__");
        assertFalse(BongAnimationRegistry.contains(probe),
            "探针 id " + probe + " 不应被任何源命中——若命中说明测试环境被污染，"
                + "contains 参数化 pin 的通过不再有证据力");
        assertTrue(BongAnimationRegistry.get(probe) == null,
            "探针 id " + probe + " 的 get 应返回 null");
    }
}
