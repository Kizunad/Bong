package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

public final class DeadDropBreakPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "dead_drop_ward_break");

    private static final int WARD_RGB = 0x3AA0C0;
    private static final int GAS_RGB = 0x6B8E23;
    private static final int GAS_TICKS = 10;
    private static final int GAS_PARTICLES_PER_TICK = 8;
    private static final int MAX_PENDING_GAS_BURSTS = 64;
    private static final Object GAS_LOCK = new Object();
    private static final List<GasBurst> GAS_BURSTS = new ArrayList<>();
    private static boolean tickerRegistered;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ensureTickerRegistered();
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        spawnWardBurst(client, world, ox, oy, oz, payload);
        enqueueGasBurst(ox, oy, oz);
    }

    private static synchronized void ensureTickerRegistered() {
        if (tickerRegistered) {
            return;
        }
        ClientTickEvents.END_CLIENT_TICK.register(DeadDropBreakPlayer::tickGasBursts);
        tickerRegistered = true;
    }

    private static void spawnWardBurst(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        VfxEventPayload.SpawnParticle payload
    ) {
        float[] rgb = GameplayVfxUtil.rgb(payload, WARD_RGB);
        int count = GameplayVfxUtil.count(payload, 12, 1, 32);
        int maxAge = GameplayVfxUtil.duration(payload, 20);
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy + 0.03, oz, rgb, 0.70f, maxAge, 0.60);
        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double up = -0.25 + world.random.nextDouble() * 0.50;
            double speed = 0.15 + world.random.nextDouble() * 0.04;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.runeCharSprites,
                ox, oy + 0.35, oz,
                Math.cos(theta) * speed,
                up * 0.06 + 0.05,
                Math.sin(theta) * speed,
                rgb, 0.78f, maxAge, 0.16f);
        }
    }

    private static void enqueueGasBurst(double ox, double oy, double oz) {
        synchronized (GAS_LOCK) {
            if (GAS_BURSTS.size() >= MAX_PENDING_GAS_BURSTS) {
                GAS_BURSTS.remove(0);
            }
            GAS_BURSTS.add(new GasBurst(ox, oy, oz, GAS_TICKS));
        }
    }

    static void tickGasBursts(MinecraftClient client) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        synchronized (GAS_LOCK) {
            Iterator<GasBurst> iterator = GAS_BURSTS.iterator();
            while (iterator.hasNext()) {
                GasBurst burst = iterator.next();
                spawnGasTick(client, world, burst);
                if (advanceGasBurst(burst)) {
                    iterator.remove();
                }
            }
        }
    }

    private static void spawnGasTick(MinecraftClient client, ClientWorld world, GasBurst burst) {
        float[] rgb = new float[] {
            ((GAS_RGB >> 16) & 0xFF) / 255f,
            ((GAS_RGB >> 8) & 0xFF) / 255f,
            (GAS_RGB & 0xFF) / 255f
        };
        for (int i = 0; i < GAS_PARTICLES_PER_TICK; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double radius = world.random.nextDouble() * 1.5;
            double x = burst.x + Math.cos(theta) * radius;
            double z = burst.z + Math.sin(theta) * radius;
            double speed = 0.025 + world.random.nextDouble() * 0.035;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.qiAuraSprites,
                x,
                burst.y + 0.08 + world.random.nextDouble() * 0.35,
                z,
                Math.cos(theta) * speed,
                0.01 + world.random.nextDouble() * 0.02,
                Math.sin(theta) * speed,
                rgb, 0.60f, 30, 0.18f);
        }
    }

    private static boolean advanceGasBurst(GasBurst burst) {
        burst.remainingTicks--;
        return burst.remainingTicks <= 0;
    }

    static int pendingGasBurstsForTests() {
        synchronized (GAS_LOCK) {
            return GAS_BURSTS.size();
        }
    }

    static int gasTicksForTests() {
        return GAS_TICKS;
    }

    static int maxPendingGasBurstsForTests() {
        return MAX_PENDING_GAS_BURSTS;
    }

    static void enqueueGasBurstForTests(double ox, double oy, double oz) {
        enqueueGasBurst(ox, oy, oz);
    }

    static void advanceGasBurstsForTests() {
        synchronized (GAS_LOCK) {
            Iterator<GasBurst> iterator = GAS_BURSTS.iterator();
            while (iterator.hasNext()) {
                if (advanceGasBurst(iterator.next())) {
                    iterator.remove();
                }
            }
        }
    }

    static void clearGasBurstsForTests() {
        synchronized (GAS_LOCK) {
            GAS_BURSTS.clear();
        }
    }

    private static final class GasBurst {
        private final double x;
        private final double y;
        private final double z;
        private int remainingTicks;

        private GasBurst(double x, double y, double z, int remainingTicks) {
            this.x = x;
            this.y = y;
            this.z = z;
            this.remainingTicks = remainingTicks;
        }
    }
}
