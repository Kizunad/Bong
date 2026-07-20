package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * NPC 三招专属粒子播放器 —— plan-skill-anim-fidelity-v1 §P5.1 ③。
 *
 * <p><b>为什么存在</b>：P5 之前 NPC 三招粒子全是借的——{@code heal_basic} 借医道
 * {@code bong:yidao_meridian_repair}、{@code buff_speed} 借真脉
 * {@code bong:jiemai_neutralize_dust}、{@code buff_defense} 借崩拳
 * {@code bong:burst_meridian_beng_quan}。后果有二：① NPC 施法与玩家同招粒子完全同形，
 * 旁观者分不清"对面 NPC 在补血"还是"有玩家在放医道"；② 借来的 id 命中玩家技能家族前缀，
 * 让 NPC 背景 cosmetic 误吃 Important 优先级、在拥挤 chunk 里挤掉玩家自己的技能反馈。
 *
 * <p>NPC 是 mob 实体、无 PlayerAnimator 通道（plan §8.1 #2），所以三招只有粒子 + 音效，
 * 形态按 plan 允许的"从简"处理；但 <b>id 与颜色强制独立</b>，构成绿 / 黄 / 蓝
 * 高分离色相三元组，保证远距离读招：
 * <table border="1">
 *   <caption>NPC 3 招粒子 spec</caption>
 *   <tr><th>event_id</th><th>基类</th><th>数量</th><th>lifetime</th>
 *       <th>速度/方向</th><th>颜色</th><th>spawn</th><th>贴图</th></tr>
 *   <tr><td>npc_heal_basic</td><td>{@link BongSpriteParticle}</td><td>12</td><td>40t</td>
 *       <td>上浮 0.03/t</td><td>#A8E6CF</td><td>radial 上升柱</td><td>qi_aura</td></tr>
 *   <tr><td>npc_buff_speed</td><td>{@link BongLineParticle}</td><td>12</td><td>40t</td>
 *       <td>水平外冲 0.22/t</td><td>#E3C766</td><td>radial 疾行尘</td><td>qi_aura</td></tr>
 *   <tr><td>npc_buff_defense</td><td>{@link BongSpriteParticle}</td><td>12</td><td>40t</td>
 *       <td>切向环绕 0.06/t</td><td>#5BA8C9</td><td>radial 护体环</td>
 *       <td>lingqi_ripple</td></tr>
 * </table>
 */
public final class NpcSkillAuraPlayer implements VfxPlayer {
    public static final Identifier HEAL_BASIC = new Identifier("bong", "npc_heal_basic");
    public static final Identifier BUFF_SPEED = new Identifier("bong", "npc_buff_speed");
    public static final Identifier BUFF_DEFENSE = new Identifier("bong", "npc_buff_defense");

    /** {@link VfxBootstrap} 遍历注册用。 */
    public static final Identifier[] EVENT_IDS = { HEAL_BASIC, BUFF_SPEED, BUFF_DEFENSE };

    static final int HEAL_RGB = 0xA8E6CF;
    static final int SPEED_RGB = 0xE3C766;
    static final int DEFENSE_RGB = 0x5BA8C9;

    private static final int DEFAULT_COUNT = 12;
    private static final int DEFAULT_LIFETIME_TICKS = 40;

    /**
     * 逐招形态。抽成枚举而非只在 {@link #play} 里比 id，是为了让 dispatch 与配色 spec
     * 都能在无 MinecraftClient 的单测里被断言（{@link #formFor}）。
     */
    enum Form {
        /** 回血：绕身上升的绿色光点柱。 */
        HEAL_COLUMN(HEAL_BASIC, HEAL_RGB),
        /** 加速：水平外冲的麦黄疾行尘。 */
        SPEED_DUST(BUFF_SPEED, SPEED_RGB),
        /** 护体：贴身切向环绕的青蓝护环。 */
        DEFENSE_RING(BUFF_DEFENSE, DEFENSE_RGB);

        final Identifier eventId;
        final int fallbackRgb;

        Form(Identifier eventId, int fallbackRgb) {
            this.eventId = eventId;
            this.fallbackRgb = fallbackRgb;
        }
    }

    /** event_id → 形态。未登记 id 返回 {@code null}（调用方静默跳过，绝不抛）。 */
    static Form formFor(Identifier eventId) {
        if (eventId == null) {
            return null;
        }
        for (Form form : Form.values()) {
            if (form.eventId.equals(eventId)) {
                return form;
            }
        }
        return null;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        Form form = formFor(payload.eventId());
        if (form == null) {
            // 未登记 id：静默跳过。
            return;
        }
        switch (form) {
            case HEAL_COLUMN -> playHealColumn(client, world, payload);
            case SPEED_DUST -> playSpeedDust(client, world, payload);
            case DEFENSE_RING -> playDefenseRing(client, world, payload);
        }
    }

    /** 回血：绕身上升的绿色光点柱。 */
    private static void playHealColumn(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = resolveCount(payload);
        float[] color = rgb(payload.colorRgb().orElse(HEAL_RGB));
        int lifetime = resolveLifetime(payload);

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                origin[0] + Math.cos(angle) * 0.4,
                origin[1] - 0.5 + (i % 4) * 0.22,
                origin[2] + Math.sin(angle) * 0.4,
                0.0, 0.03, 0.0
            );
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.85f);
            particle.setScalePublic(0.55f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 加速：水平外冲的麦黄疾行尘（线粒子，与另两招的点粒子形态区分）。 */
    private static void playSpeedDust(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = resolveCount(payload);
        float[] color = rgb(payload.colorRgb().orElse(SPEED_RGB));
        int lifetime = resolveLifetime(payload);

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);
            BongLineParticle particle = new BongLineParticle(
                world,
                origin[0] + cos * 0.3,
                origin[1] - 0.6,
                origin[2] + sin * 0.3,
                cos * 0.22, 0.01, sin * 0.22
            );
            particle.setLineShape(1.4, 0.5, 0.07);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.75f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 护体：贴身切向环绕的青蓝护环。 */
    private static void playDefenseRing(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = resolveCount(payload);
        float[] color = rgb(payload.colorRgb().orElse(DEFENSE_RGB));
        int lifetime = resolveLifetime(payload);

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);
            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                origin[0] + cos * 0.6,
                origin[1],
                origin[2] + sin * 0.6,
                -sin * 0.06, 0.0, cos * 0.06
            );
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.8f);
            particle.setScalePublic(0.6f);
            particle.setMaxAgePublic(lifetime);
            if (BongParticles.lingqiRippleSprites != null) {
                particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static int resolveCount(VfxEventPayload.SpawnParticle payload) {
        return clampInt(payload.count().orElse(DEFAULT_COUNT), 1, 48);
    }

    private static int resolveLifetime(VfxEventPayload.SpawnParticle payload) {
        return payload.durationTicks().orElse(DEFAULT_LIFETIME_TICKS);
    }

    private static float[] rgb(int rgb) {
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f,
        };
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
