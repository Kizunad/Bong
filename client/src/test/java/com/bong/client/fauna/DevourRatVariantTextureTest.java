package com.bong.client.fauna;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-devour-rat-model P2 —— 噬元鼠吸元档位 → 贴图变体 + emissive glow 契约测试。
 *
 * <p>直测 {@link FaunaModel#selectTexture} / {@link FaunaModel#devourRatTexture} /
 * {@link FaunaModel#glowTextureFor} 这几个**生产**纯函数（不实例化 {@link FaunaEntity}，
 * 免 MC 注册表 bootstrap），而不是在测试里另抄一份影子实现——影子实现会在生产逻辑
 * 漂走时仍然全绿。
 */
class DevourRatVariantTextureTest {

    private static final Path ASSETS_ROOT = Path.of("src", "main", "resources", "assets");

    @BeforeEach
    void resetState() {
        RatQiTierHandler.clearOnDisconnect();
    }

    private static void syncTier(int entityId, int tier) {
        String payload =
            "{\"v\":1,\"type\":\"rat_qi_tier\",\"entries\":[{\"id\":" + entityId + ",\"tier\":" + tier + "}]}";
        assertTrue(
            RatQiTierHandler.handleSync(payload, payload.getBytes().length),
            "测试前置：档位 payload 应被接受"
        );
    }

    private static Path assetFile(Identifier id) {
        return ASSETS_ROOT.resolve(id.getNamespace()).resolve(id.getPath());
    }

    // ── 档位 → 贴图变体 ──────────────────────────────────────────────────────

    @Test
    void tier_maps_to_its_own_texture_variant() {
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/devour_rat_q0.png"),
            FaunaModel.devourRatTexture(0),
            "0 档（没吸过真元）应用 q0（尾脊全黑）"
        );
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/devour_rat_q1.png"),
            FaunaModel.devourRatTexture(1),
            "1 档应用 q1（尾脊半亮）"
        );
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/devour_rat_q2.png"),
            FaunaModel.devourRatTexture(2),
            "2 档应用 q2（尾脊全亮）"
        );
    }

    @Test
    void three_tiers_map_to_three_distinct_textures() {
        Set<Identifier> textures = new HashSet<>(List.of(
            FaunaModel.devourRatTexture(0),
            FaunaModel.devourRatTexture(1),
            FaunaModel.devourRatTexture(2)
        ));
        assertEquals(3, textures.size(),
            "三档必须映射到三张不同贴图，否则「吸真元→尾巴变蓝」在视觉上不存在，实际 " + textures);
    }

    @Test
    void out_of_range_tier_is_clamped_to_existing_texture() {
        assertEquals(FaunaModel.devourRatTexture(0), FaunaModel.devourRatTexture(-5),
            "负档位夹到 q0，绝不能生成磁盘上不存在的路径");
        assertEquals(FaunaModel.devourRatTexture(RatQiTierHandler.TIER_MAX),
            FaunaModel.devourRatTexture(99),
            "越上界档位夹到 q" + RatQiTierHandler.TIER_MAX + "，否则渲染 missing texture 紫黑格");
    }

    // ── selectTexture：端到端「收到档位 → 选对贴图」 ─────────────────────────

    @Test
    void devour_rat_without_tier_payload_uses_q0() {
        assertEquals(
            FaunaModel.devourRatTexture(0),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, 4242),
            "没收到档位条目的鼠应渲染 q0（服务端只下发 tier>0，缺席即 0 档）"
        );
    }

    @Test
    void devour_rat_switches_texture_when_tier_payload_arrives() {
        int entityId = 4242;
        assertEquals(FaunaModel.devourRatTexture(0),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, entityId), "前置：初始 0 档");

        syncTier(entityId, 1);
        assertEquals(FaunaModel.devourRatTexture(1),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, entityId),
            "收到 tier=1 后应立刻切 q1——若仍是 q0，说明 selectTexture 没查 RatQiTierHandler");

        syncTier(entityId, 2);
        assertEquals(FaunaModel.devourRatTexture(2),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, entityId),
            "收到 tier=2 后应切 q2");
    }

    @Test
    void devour_rat_falls_back_to_q0_after_disconnect() {
        int entityId = 4242;
        syncTier(entityId, 2);
        RatQiTierHandler.clearOnDisconnect();
        assertEquals(FaunaModel.devourRatTexture(0),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, entityId),
            "断线清表后应回 q0，不能让上个 session 的档位串到新实体上");
    }

    @Test
    void multiple_rats_pick_their_own_variant_independently() {
        String payload = "{\"v\":1,\"type\":\"rat_qi_tier\",\"entries\":"
            + "[{\"id\":1,\"tier\":1},{\"id\":2,\"tier\":2}]}";
        RatQiTierHandler.handleSync(payload, payload.getBytes().length);

        assertEquals(FaunaModel.devourRatTexture(1),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, 1));
        assertEquals(FaunaModel.devourRatTexture(2),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, 2));
        assertEquals(FaunaModel.devourRatTexture(0),
            FaunaModel.selectTexture(FaunaVisualKind.DEVOUR_RAT, 3),
            "同屏三只鼠应各按各的档位渲染，不能互相串味");
    }

    /**
     * 档位表只对噬元鼠生效：别的物种即便 entity id 撞上也走自己的贴图。
     */
    @Test
    void tier_table_does_not_leak_into_other_species() {
        int entityId = 55;
        syncTier(entityId, 2);
        assertEquals(
            FaunaVisualKind.HYBRID_BEAST.textureId(),
            FaunaModel.selectTexture(FaunaVisualKind.HYBRID_BEAST, entityId),
            "非噬元鼠物种即便 entity id 命中档位表也必须用自己的贴图"
        );
    }

    // ── glow 贴图路径推导 ────────────────────────────────────────────────────

    @Test
    void glow_texture_is_derived_by_inserting_suffix_before_png() {
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/devour_rat_q1_glow.png"),
            FaunaModel.glowTextureFor(FaunaModel.devourRatTexture(1)),
            "glow 贴图 = 底图路径在 .png 前插 _glow"
        );
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/devour_rat_glow.png"),
            FaunaModel.glowTextureFor(FaunaVisualKind.DEVOUR_RAT.textureId()),
            "物种缺省底图的 glow 兜底路径同理推导"
        );
    }

    @Test
    void glow_texture_preserves_namespace() {
        Identifier glow = FaunaModel.glowTextureFor(new Identifier("minecraft", "textures/a/b.png"));
        assertEquals("minecraft", glow.getNamespace(), "namespace 不得被改写");
        assertEquals("textures/a/b_glow.png", glow.getPath());
    }

    @Test
    void glow_texture_for_path_without_png_suffix_appends_suffix() {
        assertEquals(
            new Identifier("bong", "textures/entity/fauna/foo_glow"),
            FaunaModel.glowTextureFor(new Identifier("bong", "textures/entity/fauna/foo")),
            "非 .png 结尾时直接追加后缀，保证恒返回一个确定路径（不返回 null）"
        );
    }

    @Test
    void glow_textures_are_distinct_per_tier() {
        Set<Identifier> glows = new HashSet<>(List.of(
            FaunaModel.glowTextureFor(FaunaModel.devourRatTexture(0)),
            FaunaModel.glowTextureFor(FaunaModel.devourRatTexture(1)),
            FaunaModel.glowTextureFor(FaunaModel.devourRatTexture(2))
        ));
        assertEquals(3, glows.size(),
            "三档各有各的 glow（蓝脊发光段数递增），共用一张等于档位在发光层上不可见");
    }

    // ── 资产真实存在（防路径 pin 全绿但磁盘没图）─────────────────────────────

    @Test
    void every_devour_rat_variant_texture_exists_on_disk() {
        for (int tier = 0; tier <= RatQiTierHandler.TIER_MAX; tier++) {
            Identifier base = FaunaModel.devourRatTexture(tier);
            assertTrue(Files.isRegularFile(assetFile(base)),
                "档位 " + tier + " 的底图应存在于 " + assetFile(base)
                    + "，缺失会让该档位的鼠渲染 missing texture");
        }
    }

    @Test
    void every_devour_rat_variant_has_a_glow_sibling_on_disk() {
        // emissive 层每帧按当前底图推导 glow 路径；任一档缺 glow 图 → 该档鼠被紫黑格盖住。
        for (int tier = 0; tier <= RatQiTierHandler.TIER_MAX; tier++) {
            Identifier glow = FaunaModel.glowTextureFor(FaunaModel.devourRatTexture(tier));
            assertTrue(Files.isRegularFile(assetFile(glow)),
                "档位 " + tier + " 的 glow 贴图应存在于 " + assetFile(glow));
        }
    }

    @Test
    void species_default_texture_and_its_glow_sibling_exist_on_disk() {
        // selectTexture 正常恒返回 q0/q1/q2，但 textureId() 兜底路径若被取到也不能缺图。
        Identifier base = FaunaVisualKind.DEVOUR_RAT.textureId();
        assertTrue(Files.isRegularFile(assetFile(base)), "物种缺省底图应存在于 " + assetFile(base));
        Identifier glow = FaunaModel.glowTextureFor(base);
        assertTrue(Files.isRegularFile(assetFile(glow)), "物种缺省 glow 应存在于 " + assetFile(glow));
    }

    // ── hasEmissiveGlow 契约 ─────────────────────────────────────────────────

    @Test
    void only_devour_rat_declares_emissive_glow() {
        assertTrue(FaunaVisualKind.DEVOUR_RAT.hasEmissiveGlow(), "噬元鼠有红眼+蓝脊发光");
        List<FaunaVisualKind> others = Arrays.stream(FaunaVisualKind.values())
            .filter(kind -> kind != FaunaVisualKind.DEVOUR_RAT)
            .filter(FaunaVisualKind::hasEmissiveGlow)
            .toList();
        assertTrue(others.isEmpty(),
            "其余物种未备 _glow 资产，声明发光会渲染紫黑格；违规物种: " + others);
    }

    /**
     * 反向不变式：任何声明 {@code hasEmissiveGlow()} 的物种，其缺省底图的 glow 兄弟
     * 必须真在磁盘上。将来给别的物种开发光时，这条会先撞红。
     */
    @Test
    void every_emissive_kind_has_glow_asset_for_its_default_texture() {
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            if (!kind.hasEmissiveGlow()) {
                continue;
            }
            Identifier glow = FaunaModel.glowTextureFor(kind.textureId());
            assertTrue(Files.isRegularFile(assetFile(glow)),
                kind + " 声明了 emissive glow，但缺 " + assetFile(glow));
        }
    }

    @Test
    void glow_texture_differs_from_base_texture() {
        for (int tier = 0; tier <= RatQiTierHandler.TIER_MAX; tier++) {
            Identifier base = FaunaModel.devourRatTexture(tier);
            assertNotEquals(base, FaunaModel.glowTextureFor(base),
                "glow 路径必须与底图不同，相同会把底图当发光层整只全亮");
        }
    }

    @Test
    void tier_texture_prefix_pin() {
        assertEquals("textures/entity/fauna/devour_rat_q", FaunaModel.DEVOUR_RAT_TIER_TEXTURE_PREFIX,
            "贴图前缀改动必须同步改资产文件名（pin 测试）");
        assertEquals("_glow", FaunaModel.GLOW_TEXTURE_SUFFIX,
            "glow 后缀改动必须同步改资产文件名；注意这不是 GeckoLib AutoGlowing 的 _glowmask 约定");
        assertFalse(FaunaModel.GLOW_TEXTURE_SUFFIX.equals("_glowmask"),
            "本仓库走自建 FaunaEmissiveGlowLayer（叠加层语义），不是 GeckoLib 的掩码语义");
    }
}
