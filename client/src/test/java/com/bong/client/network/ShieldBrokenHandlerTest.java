package com.bong.client.network;

import com.bong.client.audio.AudioRecipe;
import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.particle.ShieldWoodShatterPlayer;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P3：ShieldBrokenHandler 饱和单测。
 *
 * <p>覆盖：
 * <ul>
 *   <li>木盾 payload → toast + flash + 木破音 + 木屑粒子（bong:shield_break_wood 路径验证）</li>
 *   <li>骨盾 payload → toast + flash + 骨破音（差异化）+ 骨白粒子（fauna_bone_shatter 路径）</li>
 *   <li>未知材质降级 → fallback 木破音 + 木屑粒子</li>
 *   <li>缺字段降级（无 template_id / 无 instance_id）</li>
 *   <li>破盾后 EquippedShieldStore 清空</li>
 *   <li>toast 文案、颜色、时长精确锁住</li>
 * </ul>
 */
class ShieldBrokenHandlerTest {

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

    /** 记录被 spawnParticle 调用的所有 event id，用于粒子路径断言。 */
    private final List<Identifier> spawnedParticleEventIds = new ArrayList<>();
    private final VfxParticleBridge stubParticle = payload -> {
        spawnedParticleEventIds.add(payload.eventId());
        return true;
    };

    @BeforeEach
    void setUp() {
        ShieldBrokenHandler.setAudioBridgeForTests(stubAudio);
        ShieldBrokenHandler.setParticleBridgeForTests(stubParticle);
        EquippedShieldStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        ShieldBrokenHandler.resetAudioBridgeForTests();
        ShieldBrokenHandler.resetParticleBridgeForTests();
        EquippedShieldStore.resetForTests();
    }


    // ---- toast / flash 公共契约 ----------------------------------------

    @Test
    void woodenShield_returnsHandledDispatch() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(42L));
        assertTrue(dispatch.handled(),
            "shield_broken 应标记为 handled，实际 dispatch.handled()=false");
    }

    @Test
    void woodenShield_toastTextIs_盾已碎裂() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        ServerDataDispatch.ToastSpec toast = dispatch.alertToast()
            .orElseThrow(() -> new AssertionError("期望 alertToast 存在，但为空"));
        assertEquals("盾已碎裂", toast.text(),
            "toast 文案应为「盾已碎裂」，实际: " + toast.text());
    }

    @Test
    void woodenShield_toastColorMatchesConstant() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        int color = dispatch.alertToast().orElseThrow().color();
        assertEquals(ShieldBrokenHandler.BROKEN_TOAST_COLOR, color,
            "toast 颜色应为 0x" + Integer.toHexString(ShieldBrokenHandler.BROKEN_TOAST_COLOR)
                + "，实际: 0x" + Integer.toHexString(color));
    }

    @Test
    void woodenShield_toastDurationMatchesConstant() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        long dur = dispatch.alertToast().orElseThrow().durationMillis();
        assertEquals(ShieldBrokenHandler.BROKEN_TOAST_DURATION_MS, dur,
            "toast 时长应匹配 BROKEN_TOAST_DURATION_MS=" + ShieldBrokenHandler.BROKEN_TOAST_DURATION_MS
                + "，实际: " + dur);
    }

    @Test
    void woodenShield_visualEffectIsShieldBreakFlash() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        VisualEffectState effect = dispatch.visualEffectState()
            .orElseThrow(() -> new AssertionError("期望 visualEffectState 存在，但为空"));
        assertSame(VisualEffectState.EffectType.SHIELD_BREAK_FLASH, effect.effectType(),
            "VisualEffect 类型应为 SHIELD_BREAK_FLASH，实际: " + effect.effectType());
    }

    @Test
    void woodenShield_flashDurationMatchesConstant() {
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        long dur = dispatch.visualEffectState().orElseThrow().durationMillis();
        assertEquals(ShieldBrokenHandler.BROKEN_FLASH_DURATION_MS, dur,
            "flash 时长应为 " + ShieldBrokenHandler.BROKEN_FLASH_DURATION_MS + "ms，实际: " + dur);
    }

    // ---- 木盾材质差异化：音效 ------------------------------------------

    @Test
    void woodenShield_playsWoodBreakRecipe() {
        handle(woodenShieldJson(1L));
        assertFalse(playedRecipes.isEmpty(),
            "木盾破碎应触发音效播放，但 playedRecipes 为空");
        AudioEventPayload.PlaySoundRecipe played = playedRecipes.get(0);
        assertEquals("shield_break_wood", played.recipeId(),
            "木盾应播 shield_break_wood recipe，实际: " + played.recipeId());
    }

    @Test
    void woodenShield_recipeHasTwoLayers() {
        handle(woodenShieldJson(1L));
        AudioRecipe recipe = playedRecipes.get(0).recipe();
        assertEquals(2, recipe.layers().size(),
            "木盾 recipe 应有 2 层（zombie.break_wooden_door + item.break），实际: " + recipe.layers().size());
    }

    @Test
    void woodenShield_firstLayerIsZombieBreakWoodenDoor() {
        handle(woodenShieldJson(1L));
        var layer0 = playedRecipes.get(0).recipe().layers().get(0);
        assertEquals("entity.zombie.break_wooden_door", layer0.sound().getPath(),
            "木盾音效第 1 层应为 entity.zombie.break_wooden_door，实际: " + layer0.sound());
    }

    @Test
    void woodenShield_secondLayerIsItemBreak() {
        handle(woodenShieldJson(1L));
        var layer1 = playedRecipes.get(0).recipe().layers().get(1);
        assertEquals("entity.item.break", layer1.sound().getPath(),
            "木盾音效第 2 层应为 entity.item.break，实际: " + layer1.sound());
    }

    // ---- 骨盾材质差异化 --------------------------------------------------

    @Test
    void boneShield_toastTextIs_盾已碎裂() {
        ServerDataDispatch dispatch = handle(boneShieldJson(99L));
        assertEquals("盾已碎裂", dispatch.alertToast().orElseThrow().text());
    }

    @Test
    void boneShield_playsBoneBreakRecipe_notWoodBreak() {
        handle(boneShieldJson(99L));
        assertFalse(playedRecipes.isEmpty(),
            "骨盾破碎应触发音效播放");
        AudioEventPayload.PlaySoundRecipe played = playedRecipes.get(0);
        assertNotEquals("shield_break_wood", played.recipeId(),
            "骨盾应播骨声 recipe，不能与木盾相同（差异化要求）。实际 recipeId: " + played.recipeId());
        assertEquals("shield_break_bone", played.recipeId(),
            "骨盾应播 shield_break_bone recipe，实际: " + played.recipeId());
    }

    @Test
    void boneShield_firstLayerIsSkeletonHurt() {
        handle(boneShieldJson(99L));
        var layer0 = playedRecipes.get(0).recipe().layers().get(0);
        assertEquals("entity.skeleton.hurt", layer0.sound().getPath(),
            "骨盾音效第 1 层应为 entity.skeleton.hurt（骨质感），实际: " + layer0.sound());
    }

    @Test
    void boneShield_hasShieldBreakFlash() {
        ServerDataDispatch dispatch = handle(boneShieldJson(99L));
        assertSame(VisualEffectState.EffectType.SHIELD_BREAK_FLASH,
            dispatch.visualEffectState().orElseThrow().effectType(),
            "骨盾也应触发 SHIELD_BREAK_FLASH");
    }

    // ---- 未知材质降级 ----------------------------------------------------

    @Test
    void unknownTemplate_fallbackToWoodBreakAudio() {
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":7,\"template_id\":\"iron_shield\"}";
        handle(parseEnvelope(json));
        assertFalse(playedRecipes.isEmpty(),
            "未知材质应 fallback 播放木破音");
        assertEquals("shield_break_wood", playedRecipes.get(0).recipeId(),
            "未知 template 应 fallback 到 shield_break_wood recipe");
    }

    // ---- 缺字段降级 -----------------------------------------------------

    @Test
    void missingTemplateId_doesNotThrow_andReturnHandled() {
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":5}";
        ServerDataDispatch dispatch = handle(parseEnvelope(json));
        assertTrue(dispatch.handled(),
            "缺 template_id 时 handler 不应抛异常，并返回 handled");
    }

    @Test
    void missingInstanceId_doesNotThrow_andReturnHandled() {
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"template_id\":\"wooden_shield\"}";
        ServerDataDispatch dispatch = handle(parseEnvelope(json));
        assertTrue(dispatch.handled(),
            "缺 instance_id 时 handler 不应抛异常，并返回 handled");
    }

    @Test
    void emptyPayload_doesNotThrow() {
        String json = "{\"v\":1,\"type\":\"shield_broken\"}";
        assertDoesNotThrow(() -> handle(parseEnvelope(json)),
            "空 payload 时 handler 不应抛异常");
    }

    // ---- EquippedShieldStore 清空 ----------------------------------------

    @Test
    void woodenShield_clearsEquippedShieldStore_whenInstanceMatches() {
        EquippedShieldStore.equip(new EquippedShield(42L, "wooden_shield", 0f, 100f));
        handle(woodenShieldJson(42L));
        assertNull(EquippedShieldStore.snapshot(),
            "破盾后 EquippedShieldStore 应清空（instanceId 匹配）");
    }

    @Test
    void woodenShield_doesNotClearStore_whenInstanceMismatch() {
        // 新盾已经装备（instanceId 不同），破盾通知是老盾的 → 不应清空新盾
        EquippedShieldStore.equip(new EquippedShield(999L, "wooden_shield", 80f, 100f));
        handle(woodenShieldJson(42L));  // instance_id=42 != 999
        assertNotNull(EquippedShieldStore.snapshot(),
            "破盾 instance_id 不匹配时，不应清空当前装备的新盾");
    }

    @Test
    void boneShield_clearsEquippedShieldStore() {
        EquippedShieldStore.equip(new EquippedShield(99L, "bone_shield", 0f, 100f));
        handle(boneShieldJson(99L));
        assertNull(EquippedShieldStore.snapshot(),
            "骨盾破碎后 EquippedShieldStore 应清空");
    }

    // ---- 粒子路径：差异化 VFX（防回归：修复 issue #1/#4/#7 的死代码） -----------

    @Test
    void woodenShield_spawnsWoodShatterParticle() {
        handle(woodenShieldJson(1L));
        assertFalse(spawnedParticleEventIds.isEmpty(),
            "木盾破碎必须触发粒子 spawn（spawnParticle 必须被调用）；\n"
                + "期望: bridge.spawnParticle(eventId=bong:shield_break_wood) 被调用\n"
                + "实际: spawnedParticleEventIds=" + spawnedParticleEventIds
                + "；若空则 triggerMaterialEffects 未调用 spawnShieldParticle");
        Identifier expected = ShieldWoodShatterPlayer.EVENT_ID;
        assertTrue(
            spawnedParticleEventIds.contains(expected),
            "木盾破碎应触发 event_id=" + expected + " 的粒子；\n"
                + "实际 spawnedParticleEventIds=" + spawnedParticleEventIds
        );
    }

    @Test
    void boneShield_spawnsBoneShatterParticle_notWoodShatter() {
        handle(boneShieldJson(99L));
        assertFalse(spawnedParticleEventIds.isEmpty(),
            "骨盾破碎必须触发粒子 spawn；spawnedParticleEventIds=" + spawnedParticleEventIds);
        // 骨盾粒子以 bong:shield_break_bone 注册（非 fauna_bone_shatter 原始 id）
        Identifier expectedBone = ShieldBrokenHandler.PARTICLE_EVENT_BONE;
        Identifier woodId = ShieldWoodShatterPlayer.EVENT_ID;
        assertTrue(
            spawnedParticleEventIds.contains(expectedBone),
            "骨盾应触发 " + expectedBone + " 粒子（差异化，与 VfxBootstrap 注册键一致）；\n"
                + "实际 spawnedParticleEventIds=" + spawnedParticleEventIds
        );
        assertFalse(
            spawnedParticleEventIds.contains(woodId),
            "骨盾不应触发木屑粒子 " + woodId + "（材质差异化要求）；\n"
                + "实际 spawnedParticleEventIds=" + spawnedParticleEventIds
        );
    }

    @Test
    void unknownTemplate_fallbackSpawnsWoodParticle() {
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":7,\"template_id\":\"iron_shield\"}";
        handle(parseEnvelope(json));
        assertTrue(
            spawnedParticleEventIds.contains(ShieldWoodShatterPlayer.EVENT_ID),
            "未知材质 fallback 应触发木屑粒子；spawnedParticleEventIds=" + spawnedParticleEventIds
        );
    }

    @Test
    void woodenShield_particleAndAudio_bothTriggered() {
        // 验证四件套（toast/flash 通过 dispatch，音效+粒子通过 bridge stub）均有触发
        ServerDataDispatch dispatch = handle(woodenShieldJson(1L));
        assertTrue(dispatch.alertToast().isPresent(), "toast 应存在");
        assertTrue(dispatch.visualEffectState().isPresent(), "flash 应存在");
        assertFalse(playedRecipes.isEmpty(), "音效应有触发");
        assertFalse(spawnedParticleEventIds.isEmpty(), "粒子应有触发");
    }

    @Test
    void boneShield_particleAndAudio_bothTriggered() {
        ServerDataDispatch dispatch = handle(boneShieldJson(99L));
        assertTrue(dispatch.alertToast().isPresent(), "toast 应存在");
        assertTrue(dispatch.visualEffectState().isPresent(), "flash 应存在");
        assertFalse(playedRecipes.isEmpty(), "骨盾音效应有触发");
        assertFalse(spawnedParticleEventIds.isEmpty(), "骨盾粒子应有触发");
    }

    // ---- 辅助 -------------------------------------------------------

    private ServerDataDispatch handle(String json) {
        return handle(parseEnvelope(json));
    }

    private ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return new ShieldBrokenHandler().handle(envelope);
    }

    private static String woodenShieldJson(long instanceId) {
        return "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":" + instanceId
            + ",\"template_id\":\"wooden_shield\"}";
    }

    private static String boneShieldJson(long instanceId) {
        return "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":" + instanceId
            + ",\"template_id\":\"bone_shield\"}";
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), "parse failed: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
