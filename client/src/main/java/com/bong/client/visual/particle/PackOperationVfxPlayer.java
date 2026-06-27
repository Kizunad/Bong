package com.bong.client.visual.particle;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicLong;

/**
 * plan-tarkov-backpack-v1 P5 — 套包操作差异化视听反馈。
 *
 * <p>三类套包操作各自一个 {@link Kind}，server `handle_inventory_move` 经
 * {@code classify_pack_move} 判分支后 emit 对应 {@code bong:inventory_pack_*} vfx_event，
 * 本 player 按 event_id 派发到差异化粒子 + 内联差异化 audio recipe（落地散落 / 布料窸窣 /
 * 轻 thunk）。三者 event_id / 粒子基类 / 数量 / lifetime / 颜色 / audio recipe（层数、vanilla
 * sound、pitch）全部不同——单方向 stub 撞红（见 PackOperationAudioRecipeAssetTest）。
 *
 * <p>音效走 ScatterBurst 先例：client 内联构造 {@link AudioRecipe} 经 {@link SoundRecipePlayer}
 * 播放（server 只发粒子事件），committed JSON 资产 {@code assets/bong/audio_recipes/*.json} 作
 * 规格 + 测试对拍。
 */
public final class PackOperationVfxPlayer implements VfxPlayer {
    /** 卸下/穿戴背包件（worn → 非 worn）：落地音 + 物品散落粒子。 */
    public static final Identifier UNEQUIP_EVENT = new Identifier("bong", "inventory_pack_unequip");
    /** 装上背包件（非 worn → worn）：布料窸窣音 + 轻柔布料粒子。 */
    public static final Identifier EQUIP_EVENT = new Identifier("bong", "inventory_pack_equip");
    /** 拖入物品到穿戴 pack_ 容器：轻 thunk 音 + 小尘扑。 */
    public static final Identifier STOW_EVENT = new Identifier("bong", "inventory_pack_stow");

    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(43_000L);

    public enum Kind {
        UNEQUIP("inventory_pack_unequip"),
        EQUIP("inventory_pack_equip"),
        STOW("inventory_pack_stow");

        final String recipeId;

        Kind(String recipeId) {
            this.recipeId = recipeId;
        }
    }

    private final Kind kind;

    public PackOperationVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        switch (kind) {
            case UNEQUIP -> spawnUnequip(client, world, ox, oy, oz, payload);
            case EQUIP -> spawnEquip(client, world, ox, oy, oz, payload);
            case STOW -> spawnStow(client, world, ox, oy, oz, payload);
        }

        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            kind.recipeId,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.of(new AudioPosition((int) Math.floor(ox), (int) Math.floor(oy), (int) Math.floor(oz))),
            Optional.empty(),
            1.0f,
            0.0f,
            audioRecipe(kind)
        ));
    }

    /** 卸背包：暗草褐木屑向四周 + 向下散落（背包砸地连货散开）。 */
    private void spawnUnequip(
        MinecraftClient client, ClientWorld world, double ox, double oy, double oz,
        VfxEventPayload.SpawnParticle payload
    ) {
        float[] rgb = GameplayVfxUtil.rgb(payload, 0x7A6A3A);
        int count = GameplayVfxUtil.count(payload, 16, 1, 28);
        int maxAge = GameplayVfxUtil.duration(payload, 22);
        for (int i = 0; i < count; i++) {
            double theta = (Math.PI * 2.0 * i) / count + (world.random.nextDouble() - 0.5) * 0.4;
            double speed = 0.06 + world.random.nextDouble() * 0.08;
            double vy = -0.03 - world.random.nextDouble() * 0.06;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.woodDebrisSprites,
                ox,
                oy - 0.45,
                oz,
                Math.cos(theta) * speed,
                vy,
                Math.sin(theta) * speed,
                rgb,
                0.85f,
                maxAge,
                0.17f
            );
        }
    }

    /** 装背包：柔草绿尘绕胸前轻轻上飘（布料窸窣）。 */
    private void spawnEquip(
        MinecraftClient client, ClientWorld world, double ox, double oy, double oz,
        VfxEventPayload.SpawnParticle payload
    ) {
        float[] rgb = GameplayVfxUtil.rgb(payload, 0x9CA87E);
        int count = GameplayVfxUtil.count(payload, 8, 1, 16);
        int maxAge = GameplayVfxUtil.duration(payload, 14);
        for (int i = 0; i < count; i++) {
            double theta = (Math.PI * 2.0 * i) / count;
            double r = 0.18 + world.random.nextDouble() * 0.12;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.cloudDustSprites,
                ox + Math.cos(theta) * r,
                oy,
                oz + Math.sin(theta) * r,
                Math.cos(theta) * 0.01,
                0.02 + world.random.nextDouble() * 0.02,
                Math.sin(theta) * 0.01,
                rgb,
                0.55f,
                maxAge,
                0.12f
            );
        }
    }

    /** 拖入：浅褐小尘扑一下（物品入包轻顿）。 */
    private void spawnStow(
        MinecraftClient client, ClientWorld world, double ox, double oy, double oz,
        VfxEventPayload.SpawnParticle payload
    ) {
        float[] rgb = GameplayVfxUtil.rgb(payload, 0xB0A878);
        int count = GameplayVfxUtil.count(payload, 5, 1, 10);
        int maxAge = GameplayVfxUtil.duration(payload, 10);
        for (int i = 0; i < count; i++) {
            double theta = (Math.PI * 2.0 * i) / count;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.cloudDustSprites,
                ox,
                oy,
                oz,
                Math.cos(theta) * 0.02,
                0.015,
                Math.sin(theta) * 0.02,
                rgb,
                0.45f,
                maxAge,
                0.10f
            );
        }
    }

    /**
     * 三类差异化 audio recipe（层数 / vanilla sound / pitch / priority 全部不同）。
     * 必须与 {@code assets/bong/audio_recipes/inventory_pack_*.json} 逐字段对拍
     * （PackOperationAudioRecipeAssetTest 锁住）。
     */
    static AudioRecipe audioRecipe(Kind kind) {
        return switch (kind) {
            case UNEQUIP -> new AudioRecipe(
                "inventory_pack_unequip",
                List.of(
                    new AudioLayer(new Identifier("minecraft", "block.bamboo.break"), 0.6f, 0.8f, 0),
                    new AudioLayer(new Identifier("minecraft", "block.grass.break"), 0.5f, 0.9f, 2),
                    new AudioLayer(new Identifier("minecraft", "block.gravel.break"), 0.45f, 0.7f, 3)
                ),
                Optional.empty(),
                60,
                AudioAttenuation.WORLD_3D,
                AudioCategory.BLOCKS,
                AudioBus.ENVIRONMENT
            );
            case EQUIP -> new AudioRecipe(
                "inventory_pack_equip",
                List.of(
                    new AudioLayer(new Identifier("minecraft", "block.wool.place"), 0.55f, 1.0f, 0),
                    new AudioLayer(new Identifier("minecraft", "item.armor.equip_leather"), 0.6f, 1.1f, 1)
                ),
                Optional.empty(),
                55,
                AudioAttenuation.WORLD_3D,
                AudioCategory.BLOCKS,
                AudioBus.ENVIRONMENT
            );
            case STOW -> new AudioRecipe(
                "inventory_pack_stow",
                List.of(
                    new AudioLayer(new Identifier("minecraft", "block.wool.hit"), 0.5f, 1.2f, 0),
                    new AudioLayer(new Identifier("minecraft", "entity.item.pickup"), 0.35f, 0.9f, 1)
                ),
                Optional.empty(),
                50,
                AudioAttenuation.WORLD_3D,
                AudioCategory.BLOCKS,
                AudioBus.ENVIRONMENT
            );
        };
    }
}
