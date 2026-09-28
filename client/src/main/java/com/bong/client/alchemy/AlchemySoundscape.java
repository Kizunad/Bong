package com.bong.client.alchemy;

import net.minecraft.client.world.ClientWorld;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;
import net.minecraft.util.math.Vec3d;

/** 持续炉火只在服务端心跳存续时播放短声；操作音效由 audio/play 独立接收。 */
public final class AlchemySoundscape {
    private AlchemySoundscape() {}

    public static void playFire(ClientWorld world, Vec3d center, float heat) {
        world.playSound(center.x, center.y + .3, center.z, SoundEvents.BLOCK_CAMPFIRE_CRACKLE,
            SoundCategory.BLOCKS, .12f + heat * .35f, .7f + heat * .35f, false);
    }
}
