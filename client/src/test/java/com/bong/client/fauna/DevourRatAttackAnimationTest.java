package com.bong.client.fauna;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-devour-rat-model P3 —— 噬元鼠招式动画的**客户端半边**契约。
 *
 * <p>服务端 {@code network::rat_av_trigger} 往 wire 上写死这三个动画名与时长；GeckoLib
 * 解析不到 key 时不会报错，只会让实体定格绑定姿势（俗称 T-Pose）。故必须在客户端侧锁住：
 * ① 三个 key 真在 {@code devour_rat.animation.json} 里；② 都是一次性动画（非 loop，
 * 否则 {@code FaunaActionAnimation} 倒计时结束前不会回 idle 就没意义了）；
 * ③ 动画时长与服务端下发的 {@code duration_ticks} 一致（server 短了会截断、长了会僵住）。
 */
class DevourRatAttackAnimationTest {

    /**
     * 服务端 {@code rat_av_trigger.rs} 的 (动画名 → duration_ticks) 映射快照。
     *
     * <p>跨仓库契约 pin：动画资产改了 length 就必须同步改服务端常量，否则这里撞红。
     */
    private static final Map<String, Integer> SERVER_ATTACK_ANIM_TICKS = new LinkedHashMap<>();

    static {
        // RatAttackKind::Peck   → TICKS_DEVOUR_RAT_PECK
        SERVER_ATTACK_ANIM_TICKS.put("animation.bong.devour_rat.peck", 10);
        // RatAttackKind::Claw   → TICKS_DEVOUR_RAT_CLAW
        SERVER_ATTACK_ANIM_TICKS.put("animation.bong.devour_rat.claw", 12);
        // RatAttackKind::Pounce → TICKS_DEVOUR_RAT_POUNCE
        SERVER_ATTACK_ANIM_TICKS.put("animation.bong.devour_rat.pounce", 16);
    }

    private static final int TICKS_PER_SECOND = 20;

    private static JsonObject devourRatAnimations() throws IOException {
        Identifier animId = FaunaVisualKind.DEVOUR_RAT.animationId();
        Path path = Path.of("src", "main", "resources", "assets", animId.getNamespace())
            .resolve(animId.getPath());
        assertTrue(Files.isRegularFile(path), "噬元鼠动画文件应存在于 " + path);
        JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
        JsonObject animations = root.getAsJsonObject("animations");
        assertTrue(animations != null && !animations.keySet().isEmpty(),
            "动画文件应含非空 animations 对象");
        return animations;
    }

    @Test
    void all_server_triggered_attack_animations_exist() throws IOException {
        JsonObject animations = devourRatAnimations();
        for (String animName : SERVER_ATTACK_ANIM_TICKS.keySet()) {
            assertTrue(
                animations.has(animName),
                "服务端 rat_av_trigger 会下发 \"" + animName + "\"，但 devour_rat.animation.json 里没有此 key"
                    + "——GeckoLib 解析不到不会报错，只会让鼠定格绑定姿势（T-Pose）。文件含: "
                    + animations.keySet()
            );
        }
    }

    @Test
    void attack_animations_are_one_shot_not_looping() throws IOException {
        JsonObject animations = devourRatAnimations();
        for (String animName : SERVER_ATTACK_ANIM_TICKS.keySet()) {
            JsonObject anim = animations.getAsJsonObject(animName);
            boolean looped = anim.has("loop") && anim.get("loop").getAsBoolean();
            assertFalse(
                looped,
                animName + " 必须是一次性动画（不带 loop:true）——招式动画由 FaunaActionAnimation "
                    + "倒计时驱动，循环动画会在倒计时结束前反复重播"
            );
        }
    }

    @Test
    void attack_animation_lengths_match_server_duration_ticks() throws IOException {
        JsonObject animations = devourRatAnimations();
        for (Map.Entry<String, Integer> entry : SERVER_ATTACK_ANIM_TICKS.entrySet()) {
            String animName = entry.getKey();
            int serverTicks = entry.getValue();
            double lengthSeconds = animations.getAsJsonObject(animName)
                .get("animation_length").getAsDouble();
            int expectedTicks = (int) Math.ceil(lengthSeconds * TICKS_PER_SECOND);
            assertEquals(
                expectedTicks,
                serverTicks,
                animName + " 资产长度 " + lengthSeconds + "s = ceil(×20) = " + expectedTicks
                    + " tick，服务端 duration_ticks 写的是 " + serverTicks
                    + "——短了动画被截断，长了播完后鼠僵在最后一帧才回 idle"
            );
        }
    }

    /** idle 必须是循环动画（与招式相反）：否则播完一轮就静止。 */
    @Test
    void idle_animation_is_looping() throws IOException {
        JsonObject animations = devourRatAnimations();
        String idleName = FaunaVisualKind.DEVOUR_RAT.idleAnimationName();
        assertTrue(animations.has(idleName), "idle 动画 " + idleName + " 应存在");
        JsonObject idle = animations.getAsJsonObject(idleName);
        assertTrue(
            idle.has("loop") && idle.get("loop").getAsBoolean(),
            idleName + " 必须 loop:true，否则鼠站着播一轮就定住"
        );
    }

    /** 三招动画必须真的动到不同骨骼组合，否则玩家分辨不出用的是哪一招。 */
    @Test
    void three_attack_animations_animate_distinct_bone_sets() throws IOException {
        JsonObject animations = devourRatAnimations();
        Map<String, java.util.Set<String>> boneSets = new LinkedHashMap<>();
        for (String animName : SERVER_ATTACK_ANIM_TICKS.keySet()) {
            JsonObject bones = animations.getAsJsonObject(animName).getAsJsonObject("bones");
            assertTrue(bones != null && !bones.keySet().isEmpty(), animName + " 必须至少动一根骨骼");
            boneSets.put(animName, bones.keySet());
        }
        assertEquals(
            3,
            new java.util.HashSet<>(boneSets.values()).size(),
            "peck/claw/pounce 应各动不同骨骼组合（啄=头/身、抓=前爪、扑=四腿+身），"
                + "完全相同意味着三招在视觉上无法分辨。实际: " + boneSets
        );
    }
}
