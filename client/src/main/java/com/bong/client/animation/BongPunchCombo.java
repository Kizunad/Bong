package com.bong.client.animation;

import com.bong.client.BongClient;
import com.bong.client.BongClientFeatures;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.VisualEffectController;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;

/**
 * 右直拳"组合拳"示例：把 {@link BongAnimations#FIST_PUNCH_RIGHT} 动画 +
 * 屏幕微震动（SCREEN_SHAKE 低强度）+ 拳风声（attack.sweep）+ 拳风粒子
 * （SWEEP_ATTACK + CLOUD）串成一次 {@link #trigger} 调用。
 *
 * <p>这是 plan-player-animation-v1 §5.1 与 plan-vfx-v1 / plan-particle-system-v1
 * 的整合 demo：验证"一次触发 → 多层效果同步"的客户端编排路径。server→client
 * 协议（§4.1 play_anim）之后可直接调用 {@link #trigger} 承接。
 *
 * <p><b>时序</b>（tick / 1 tick = 50ms，动画总长 8 tick = 0.4s）：
 * <ul>
 *   <li>tick 0：{@link BongAnimationPlayer#play} 启动动画，同时把 shake/sound/
 *       particles 推迟到 impact 帧</li>
 *   <li>tick 1→3：anticipation + 蓄力（躯干扭、右腿蹲），视觉静默</li>
 *   <li>tick 5（impact）：同步触发 SCREEN_SHAKE(0.4, 250ms) + attack.sweep
 *       + SWEEP_ATTACK×1 + CLOUD×5，肉眼上会感知成"蓄势 → 拳砸 → 画面抖 →
 *       拳风声 → 粒子扩散"的全身发力复合动作</li>
 *   <li>tick 6→8：overshoot + 收拳回位，组合拳结束</li>
 * </ul>
 *
 * <p><b>已知限制</b>（demo 阶段可接受）：SCREEN_SHAKE profile 的 retrigger 窗口
 * 是 1200ms（见 {@code VisualEffectProfile.SYSTEM_WARNING}），所以 1.2s 内连击
 * 第二拳的屏幕震动会被吞掉；动画本身没有 retrigger 限制，仍然连播。若要让连拳
 * 抖动也跟上，需要加一个专用 PUNCH_SHAKE profile（retrigger ~100ms）——留给
 * 后续。
 */
public final class BongPunchCombo {
    /** impact 帧：动画 v3 从 6 tick 延长到 8 tick，impact 也从 tick 3 后移到 tick 5
     *  —— 前面 tick 1 anticipation + tick 3 蓄力极限，tick 5 才是拳到最远点。 */
    private static final int IMPACT_TICK = 5;
    /** 屏幕震动强度：0.4 对应 ~0.6° 振幅，"一闪"级别，不晕头。 */
    private static final double SHAKE_INTENSITY = 0.4;
    /** 震动时长：250ms ≈ 5 tick，和 impact 后的 3 tick overshoot+recovery 基本重叠，
     *  震动感和收拳动作同步淡出。 */
    private static final long SHAKE_DURATION_MS = 250L;
    /** 拳风声音量 / 音高：音高 1.1f 稍脆，不和 vanilla sweep 混淆。 */
    private static final float PUNCH_SOUND_VOLUME = 0.9f;
    private static final float PUNCH_SOUND_PITCH = 1.1f;

    /** 延时任务队列：tick 倒数到 0 就执行 {@link Delayed#action}。 */
    private static final List<Delayed> PENDING = Collections.synchronizedList(new ArrayList<>());

    private BongPunchCombo() {
    }

    /** 客户端启动时调用一次：挂一个 {@link ClientTickEvents#END_CLIENT_TICK} 消费队列。 */
    public static void bootstrap() {
        ClientTickEvents.END_CLIENT_TICK.register(client -> tickPending());
    }

    /**
     * 触发一次右直拳组合拳。
     *
     * @return true=动画正常启动；false=玩家/动画 id 无效，此时 shake/sound/particles 也不会发
     */
    public static boolean trigger(AbstractClientPlayerEntity player) {
        if (player == null) {
            return false;
        }
        // 3 tick fade-in：vanilla 下垂 → guard pose 差距较大（左臂 pitch 0→-45°、
        // bend 0→90°），1 tick 会让观众看到"左臂嗖一下抬起就定住"的僵硬感；3 tick
        // 过渡让抬起过程平滑，配合 v3.4 左臂 load-snap 弹性动态，整体节奏像呼吸而非木偶
        boolean ok = BongAnimationPlayer.play(player, BongAnimations.FIST_PUNCH_RIGHT, 1000, 3);
        if (!ok) {
            return false;
        }
        // 所有"碰撞瞬间"效果集中挂在 IMPACT_TICK
        scheduleAfter(IMPACT_TICK, () -> fireImpact(player));
        return true;
    }

    /** impact 帧的四件套：屏幕震 + 拳风声 + 弧刃粒子 + 拳风烟。 */
    private static void fireImpact(AbstractClientPlayerEntity player) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.world == null) {
            return;
        }

        // 1) 屏幕微震动（走既有 VFX store，让 MixinCamera 复用 CameraShakeOffsets）
        triggerScreenShake();

        // 2) 拳风声：vanilla 挥砍 whoosh，SoundCategory.PLAYERS 让音量走玩家音轨
        player.playSound(
            SoundEvents.ENTITY_PLAYER_ATTACK_SWEEP,
            SoundCategory.PLAYERS,
            PUNCH_SOUND_VOLUME,
            PUNCH_SOUND_PITCH
        );

        // 3) 粒子：在"右拳大致位置"（眼前 1 格、右 0.3、下 0.4）爆一朵
        spawnPunchParticles(client.world, player);
    }

    /** 复制 {@code BongVfxCommand.executeTrigger} 的 VFX 推送管线，只是参数固定。 */
    private static void triggerScreenShake() {
        long now = System.currentTimeMillis();
        VisualEffectState incoming = VisualEffectState.create(
            "screen_shake", SHAKE_INTENSITY, SHAKE_DURATION_MS, now
        );
        if (incoming.isEmpty()) {
            BongClient.LOGGER.warn("[bong/combo] screen_shake state 构造失败，跳过震动");
            return;
        }
        BongHudStateSnapshot current = BongHudStateStore.snapshot();
        VisualEffectState next = VisualEffectController.acceptIncoming(
            current.visualEffectState(),
            incoming,
            now,
            BongClientFeatures.ENABLE_VISUAL_EFFECTS
        );
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            current.zoneState(),
            current.narrationState(),
            next
        ));
    }

    /**
     * 在玩家"拳头预估位置"喷拳风粒子：
     * <ul>
     *   <li>1 个 {@code SWEEP_ATTACK}——vanilla 白色弧刃，形状上天然像拳风的"气爆"</li>
     *   <li>5 个 {@code CLOUD}——带前向速度的小烟团，做出"空气被击碎扩散"的残留</li>
     * </ul>
     * 这个位置是粗略估计：用玩家 look 方向投影，没查骨骼实际世界坐标，因为 PlayerAnimator
     * 的骨骼位置需要跨 MatrixStack 提取，对 demo 来说成本过高。实际 ~1 格前方已经够"看起来
     * 像在拳头那里"。
     */
    private static void spawnPunchParticles(ClientWorld world, AbstractClientPlayerEntity player) {
        Vec3d eye = player.getCameraPosVec(1.0f);
        Vec3d look = player.getRotationVector();
        // 右手垂直分量：水平面内 look 向量顺时针 90°（MC 坐标系 -z 为北，右侧对应 -z 旋 x）
        Vec3d right = new Vec3d(-look.z, 0.0, look.x);
        double rightLen = right.length();
        if (rightLen > 1e-6) {
            right = right.multiply(1.0 / rightLen);
        } else {
            right = new Vec3d(1, 0, 0);
        }

        Vec3d fist = eye
            .add(look.multiply(1.0))       // 前 1 格
            .add(right.multiply(0.3))      // 右 0.3 格（模拟右臂偏外）
            .add(0, -0.4, 0);              // 下沉 0.4 格（拳在眼下，不在视线上）

        // 一刀弧：vx/vy/vz 传 0，vanilla SWEEP_ATTACK 不依赖速度，朝向由摄像机决定
        world.addParticle(ParticleTypes.SWEEP_ATTACK, fist.x, fist.y, fist.z, 0.0, 0.0, 0.0);

        // 拳风烟：5 个带前向初速的 CLOUD，做出"爆气扩散"
        for (int i = 0; i < 5; i++) {
            double jitterX = (world.random.nextDouble() - 0.5) * 0.25;
            double jitterY = (world.random.nextDouble() - 0.5) * 0.25;
            double jitterZ = (world.random.nextDouble() - 0.5) * 0.25;
            double vx = look.x * 0.18 + jitterX * 0.3;
            double vy = look.y * 0.12 + jitterY * 0.3;
            double vz = look.z * 0.18 + jitterZ * 0.3;
            world.addParticle(
                ParticleTypes.CLOUD,
                fist.x + jitterX,
                fist.y + jitterY,
                fist.z + jitterZ,
                vx, vy, vz
            );
        }
    }

    /** 给 {@link #trigger} 把 impact 效果延后到动画第 N tick 再发。 */
    private static void scheduleAfter(int ticks, Runnable action) {
        if (ticks <= 0) {
            try {
                action.run();
            } catch (Throwable t) {
                BongClient.LOGGER.warn("[bong/combo] 即时 action 抛错", t);
            }
            return;
        }
        PENDING.add(new Delayed(ticks, action));
    }

    /** 每 client tick 扣 1；到期的拿出来跑，跑完从队列移除。 */
    private static void tickPending() {
        synchronized (PENDING) {
            if (PENDING.isEmpty()) {
                return;
            }
            Iterator<Delayed> it = PENDING.iterator();
            while (it.hasNext()) {
                Delayed d = it.next();
                d.remainingTicks--;
                if (d.remainingTicks <= 0) {
                    it.remove();
                    try {
                        d.action.run();
                    } catch (Throwable t) {
                        BongClient.LOGGER.warn("[bong/combo] delayed action 抛错", t);
                    }
                }
            }
        }
    }

    /** 可变 remainingTicks 的小容器——record 不能 mutate，这里只能用普通类。 */
    private static final class Delayed {
        int remainingTicks;
        final Runnable action;

        Delayed(int ticks, Runnable action) {
            this.remainingTicks = ticks;
            this.action = action;
        }
    }
}
