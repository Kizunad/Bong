package com.bong.client.network;

import com.bong.client.audio.AudioRecipe;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P4：ShieldBlockHitHandler 饱和单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>木盾 → 木盾音效（pitch 0.9）+ 木盾粒子（shield_block_wood）</li>
 *   <li>骨盾 → 骨盾音效（pitch 1.3 + skeleton.hurt 第二层）+ 骨盾粒子（shield_block_bone）</li>
 *   <li>未知材质降级 → 木盾效果</li>
 *   <li>材质路由正确：木≠骨 pitch 差异化断言</li>
 *   <li>HUD toast 文案 + 颜色正确</li>
 *   <li>dispatch.handled() 断言</li>
 *   <li>粒子 count/duration 命中迸溅轻量参数（与 P3 破碎区分）</li>
 * </ul>
 */
class ShieldBlockHitHandlerTest {

    private final List<AudioEventPayload.PlaySoundRecipe> playedRecipes = new ArrayList<>();
    private final AudioPlaybackBridge stubAudio = new AudioPlaybackBridge() {
        @Override
        public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
            playedRecipes.add(payload);
            return true;
        }

        @Override
        public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
            return false;
        }
    };

    private final List<VfxEventPayload.SpawnParticle> spawnedParticles = new ArrayList<>();
    private final VfxParticleBridge stubParticle = payload -> {
        spawnedParticles.add(payload);
        return true;
    };

    @BeforeEach
    void setUp() {
        ShieldBlockHitHandler.setAudioBridgeForTests(stubAudio);
        ShieldBlockHitHandler.setParticleBridgeForTests(stubParticle);
    }

    @AfterEach
    void tearDown() {
        ShieldBlockHitHandler.resetAudioBridgeForTests();
        ShieldBlockHitHandler.resetParticleBridgeForTests();
    }

    // ── dispatch 公共契约 ────────────────────────────────────────────────

    @Test
    void woodenShield_returnsHandledDispatch() {
        ServerDataDispatch dispatch = handle(woodenShieldJson());
        assertTrue(dispatch.handled(),
            "shield_block_hit(wooden_shield) 应标记 handled，实际 dispatch.handled()=false");
    }

    @Test
    void boneShield_returnsHandledDispatch() {
        ServerDataDispatch dispatch = handle(boneShieldJson());
        assertTrue(dispatch.handled(),
            "shield_block_hit(bone_shield) 应标记 handled，实际 dispatch.handled()=false");
    }

    // ── HUD toast 规格 ────────────────────────────────────────────────────

    @Test
    void woodenShield_toastColor_isBlockHudColor() {
        ServerDataDispatch dispatch = handle(woodenShieldJson());
        assertTrue(dispatch.alertToast().isPresent(),
            "shield_block_hit(wooden_shield) 应有 alertToast，实际 alertToast absent");
        assertEquals(
            ShieldBlockHitHandler.BLOCK_HUD_COLOR,
            dispatch.alertToast().get().color(),
            "期望格挡 HUD toast 颜色=" + Integer.toHexString(ShieldBlockHitHandler.BLOCK_HUD_COLOR)
                + " 区别于普通受击红，实际颜色=" + Integer.toHexString(dispatch.alertToast().get().color())
        );
    }

    @Test
    void boneShield_toastText_containsBlockLabel() {
        ServerDataDispatch dispatch = handle(boneShieldJson());
        assertTrue(dispatch.alertToast().isPresent(),
            "shield_block_hit(bone_shield) 应有 alertToast，实际 absent");
        String text = dispatch.alertToast().get().text();
        assertFalse(text.isBlank(),
            "骨盾格挡 toast 文案不应为空");
    }

    @Test
    void toastDuration_isPositive() {
        ServerDataDispatch dispatch = handle(woodenShieldJson());
        assertTrue(dispatch.alertToast().isPresent());
        assertTrue(dispatch.alertToast().get().durationMillis() > 0,
            "格挡 toast 时长应 > 0，实际 durationMillis=" + dispatch.alertToast().get().durationMillis());
    }

    // ── 木盾音效规格 ─────────────────────────────────────────────────────

    @Test
    void woodenShield_playsOneAudioRecipe() {
        handle(woodenShieldJson());
        assertEquals(1, playedRecipes.size(),
            "木盾格挡命中应播 1 个 recipe，实际 playedRecipes.size()=" + playedRecipes.size());
    }

    @Test
    void woodenShield_audioRecipeId_isShieldBlock() {
        handle(woodenShieldJson());
        assertEquals("shield_block", playedRecipes.get(0).recipeId(),
            "木盾 recipe id 应为 shield_block，实际=" + playedRecipes.get(0).recipeId());
    }

    @Test
    void woodenShield_audioLayer1_pitchIs0point9() {
        handle(woodenShieldJson());
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(1, recipe.layers().size(),
            "木盾 shield_block recipe 应有 1 layer，实际 layers=" + recipe.layers().size());
        assertEquals(0.9f, recipe.layers().get(0).pitch(), 0.001f,
            "木盾 layer1 pitch 应 0.9（与骨盾 1.3 形成可感知差异），实际 pitch=" + recipe.layers().get(0).pitch());
    }

    @Test
    void woodenShield_audioLayer1_soundIsShieldBlock() {
        handle(woodenShieldJson());
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(
            new Identifier("minecraft", "item.shield.block"),
            recipe.layers().get(0).sound(),
            "木盾 layer1 sound 应为 minecraft:item.shield.block"
        );
    }

    // ── 骨盾音效规格 ─────────────────────────────────────────────────────

    @Test
    void boneShield_playsOneAudioRecipe() {
        handle(boneShieldJson());
        assertEquals(1, playedRecipes.size(),
            "骨盾格挡命中应播 1 个 recipe，实际 playedRecipes.size()=" + playedRecipes.size());
    }

    @Test
    void boneShield_audioRecipeId_isShieldBlockBone() {
        handle(boneShieldJson());
        assertEquals("shield_block_bone", playedRecipes.get(0).recipeId(),
            "骨盾 recipe id 应为 shield_block_bone，实际=" + playedRecipes.get(0).recipeId());
    }

    @Test
    void boneShield_audioLayer1_pitchIs1point3() {
        handle(boneShieldJson());
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(0.8f, recipe.layers().get(0).volume(), 0.001f,
            "骨盾 layer1 volume 应 0.8，实际=" + recipe.layers().get(0).volume());
        assertEquals(1.3f, recipe.layers().get(0).pitch(), 0.001f,
            "骨盾 layer1 pitch 应 1.3（高于木盾 0.9，骨感脆响），实际=" + recipe.layers().get(0).pitch());
    }

    @Test
    void boneShield_hasTwoLayers() {
        handle(boneShieldJson());
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(2, recipe.layers().size(),
            "骨盾 shield_block_bone recipe 应有 2 layers（layer2=skeleton.hurt），实际=" + recipe.layers().size());
    }

    @Test
    void boneShield_audioLayer2_isSkeletonHurt() {
        handle(boneShieldJson());
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(
            new Identifier("minecraft", "entity.skeleton.hurt"),
            recipe.layers().get(1).sound(),
            "骨盾 layer2 sound 应为 minecraft:entity.skeleton.hurt（骨感回响），实际=" + recipe.layers().get(1).sound()
        );
        assertEquals(0.3f, recipe.layers().get(1).volume(), 0.001f,
            "骨盾 layer2 volume 应 0.3，实际=" + recipe.layers().get(1).volume());
        assertEquals(1, recipe.layers().get(1).delayTicks(),
            "骨盾 layer2 delay_ticks 应 1，实际=" + recipe.layers().get(1).delayTicks());
    }

    // ── 材质差异化关键断言 ────────────────────────────────────────────────

    @Test
    void woodVsBone_pitchAreDifferentAndDistinguishable() {
        handle(woodenShieldJson());
        float woodPitch = playedRecipes.get(0).recipe().layers().get(0).pitch();
        playedRecipes.clear();
        handle(boneShieldJson());
        float bonePitch = playedRecipes.get(0).recipe().layers().get(0).pitch();

        assertNotEquals(woodPitch, bonePitch, 0.001f,
            "木盾 pitch=" + woodPitch + " 与骨盾 pitch=" + bonePitch + " 应不同（材质差异化），实际相同");
        assertTrue(bonePitch > woodPitch,
            "骨盾 pitch(" + bonePitch + ") 应 > 木盾 pitch(" + woodPitch + ")（骨盾脆响更高），实际相反");
    }

    @Test
    void woodVsBone_layerCountDiffers() {
        handle(woodenShieldJson());
        int woodLayers = playedRecipes.get(0).recipe().layers().size();
        playedRecipes.clear();
        handle(boneShieldJson());
        int boneLayers = playedRecipes.get(0).recipe().layers().size();

        assertEquals(1, woodLayers,
            "木盾应只有 1 layer（无 skeleton.hurt 附加层），实际=" + woodLayers);
        assertEquals(2, boneLayers,
            "骨盾应有 2 layers（layer2=skeleton.hurt），实际=" + boneLayers);
    }

    // ── 粒子规格 ──────────────────────────────────────────────────────────

    @Test
    void woodenShield_spawnsWoodParticle() {
        handle(woodenShieldJson());
        assertEquals(1, spawnedParticles.size(),
            "木盾格挡命中应触发 1 次粒子，实际=" + spawnedParticles.size());
        assertEquals(
            ShieldBlockHitHandler.PARTICLE_BLOCK_WOOD,
            spawnedParticles.get(0).eventId(),
            "木盾粒子 eventId 应为 bong:shield_block_wood，实际=" + spawnedParticles.get(0).eventId()
        );
    }

    @Test
    void boneShield_spawnsBoNEParticle() {
        handle(boneShieldJson());
        assertEquals(1, spawnedParticles.size(),
            "骨盾格挡命中应触发 1 次粒子，实际=" + spawnedParticles.size());
        assertEquals(
            ShieldBlockHitHandler.PARTICLE_BLOCK_BONE,
            spawnedParticles.get(0).eventId(),
            "骨盾粒子 eventId 应为 bong:shield_block_bone，实际=" + spawnedParticles.get(0).eventId()
        );
    }

    @Test
    void woodenShield_particleCount_isLightweight6() {
        handle(woodenShieldJson());
        VfxEventPayload.SpawnParticle particle = spawnedParticles.get(0);
        assertTrue(
            particle.count().isPresent(),
            "木盾格挡命中粒子应指定 count（P4 命中轻量 6 颗），实际 count absent"
        );
        assertEquals(6, particle.count().getAsInt(),
            "木盾命中迸溅粒子 count 应=6（区别于 P3 破碎 count=7），实际=" + particle.count().getAsInt()
        );
    }

    @Test
    void woodenShield_particleDuration_isLightweight8() {
        handle(woodenShieldJson());
        VfxEventPayload.SpawnParticle particle = spawnedParticles.get(0);
        assertTrue(
            particle.durationTicks().isPresent(),
            "木盾格挡命中粒子应指定 durationTicks（P4 命中轻量 8t），实际 durationTicks absent"
        );
        assertEquals(8, particle.durationTicks().getAsInt(),
            "木盾命中迸溅粒子 duration 应=8t（区别于 P3 破碎 10t），实际=" + particle.durationTicks().getAsInt()
        );
    }

    @Test
    void woodVsBone_particleEventIdsDiffer() {
        handle(woodenShieldJson());
        Identifier woodId = spawnedParticles.get(0).eventId();
        spawnedParticles.clear();
        handle(boneShieldJson());
        Identifier boneId = spawnedParticles.get(0).eventId();

        assertNotEquals(woodId, boneId,
            "木盾粒子 eventId=" + woodId + " 与骨盾=" + boneId + " 不应相同（材质路由必须差异化）");
    }

    // ── 降级保护 ─────────────────────────────────────────────────────────

    @Test
    void unknownMaterial_fallsBackToWoodEffects() {
        handle(unknownShieldJson());
        assertFalse(playedRecipes.isEmpty(),
            "未知材质盾格挡应降级播木盾音效，实际 playedRecipes 为空");
        assertEquals("shield_block", playedRecipes.get(0).recipeId(),
            "未知材质应降级为 shield_block，实际=" + playedRecipes.get(0).recipeId());
        assertFalse(spawnedParticles.isEmpty(),
            "未知材质盾格挡应降级生成木盾粒子，实际 spawnedParticles 为空");
        assertEquals(
            ShieldBlockHitHandler.PARTICLE_BLOCK_WOOD,
            spawnedParticles.get(0).eventId(),
            "未知材质应降级为 shield_block_wood，实际=" + spawnedParticles.get(0).eventId()
        );
    }

    // ── 粒子/音效 ID 命名规范 ───────────────────────────────────────────────

    @Test
    void particleBlockWood_hasBongNamespaceAndCorrectPath() {
        assertEquals("bong", ShieldBlockHitHandler.PARTICLE_BLOCK_WOOD.getNamespace(),
            "PARTICLE_BLOCK_WOOD namespace 应为 bong");
        assertEquals("shield_block_wood", ShieldBlockHitHandler.PARTICLE_BLOCK_WOOD.getPath(),
            "PARTICLE_BLOCK_WOOD path 应为 shield_block_wood（区别于 P3 shield_break_wood）");
    }

    @Test
    void particleBlockBone_hasBongNamespaceAndCorrectPath() {
        assertEquals("bong", ShieldBlockHitHandler.PARTICLE_BLOCK_BONE.getNamespace(),
            "PARTICLE_BLOCK_BONE namespace 应为 bong");
        assertEquals("shield_block_bone", ShieldBlockHitHandler.PARTICLE_BLOCK_BONE.getPath(),
            "PARTICLE_BLOCK_BONE path 应为 shield_block_bone（区别于 P3 shield_break_bone）");
    }

    @Test
    void blockHudColor_isDifferentFromBreakToastColor() {
        // P3 BROKEN_TOAST_COLOR = 0xFFC04040（红色）
        // P4 BLOCK_HUD_COLOR = 0xFF4DA6FF（青蓝）
        // 两者不同，格挡提示颜色可感知区别于破盾红
        int brokenColor = ShieldBrokenHandler.BROKEN_TOAST_COLOR;
        assertNotEquals(brokenColor, ShieldBlockHitHandler.BLOCK_HUD_COLOR,
            "格挡命中 HUD 颜色=" + Integer.toHexString(ShieldBlockHitHandler.BLOCK_HUD_COLOR)
                + " 应区别于破盾 toast 颜色=" + Integer.toHexString(brokenColor));
    }

    // ── helpers ───────────────────────────────────────────────────────────

    private ServerDataDispatch handle(String json) {
        byte[] bytes = json.getBytes(StandardCharsets.UTF_8);
        var parseResult = ServerDataEnvelope.parse(json, bytes.length);
        assertTrue(parseResult.isSuccess(), "JSON 解析失败: " + parseResult.errorMessage());
        return new ShieldBlockHitHandler().handle(parseResult.envelope());
    }

    private static String woodenShieldJson() {
        return "{\"v\":1,\"type\":\"shield_block_hit\",\"template_id\":\"wooden_shield\"}";
    }

    private static String boneShieldJson() {
        return "{\"v\":1,\"type\":\"shield_block_hit\",\"template_id\":\"bone_shield\"}";
    }

    private static String unknownShieldJson() {
        return "{\"v\":1,\"type\":\"shield_block_hit\",\"template_id\":\"mystery_shield\"}";
    }
}
