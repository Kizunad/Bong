package com.bong.client.weapon;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.UUID;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.Perspective;
import net.minecraft.client.texture.NativeImage;
import net.minecraft.client.util.ScreenshotRecorder;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.registry.Registries;
import net.minecraft.server.integrated.IntegratedServer;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.util.Identifier;
import net.minecraft.world.GameMode;
import net.minecraft.world.GameRules;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 通过环境变量激活的客户端自动截图 harness —— 用于替代人工 runClient 验证
 * weapon display transform。
 *
 * 激活方式（典型通过 client/tools/screenshot_weapon.sh wrapper）:
 *   BONG_WEAPON_TEST_ITEM=minecraft:iron_sword     必填，/give 给玩家哪个 item
 *   BONG_WEAPON_TEST_ASSET=placeholder_sword       必填，输出子目录名
 *   BONG_WEAPON_TEST_OUT=client/tools/renders      可选，输出根目录，默认即此
 *   BONG_WEAPON_TEST_BONG_EQUIP=iron_sword         可选，设了则不 /give vanilla item,
 *                                                  直接塞 WeaponEquippedStore 模拟
 *                                                  server 推 WeaponEquippedV1,测
 *                                                  §5.1 MixinHeldItemRenderer 路径
 *
 * 之所以用 env var 而非 -D syspro：gradle runClient → JavaExec 子进程，env var 默认
 * 被继承；-D 要改 build.gradle 加转发钩子。
 *
 * 用法前置条件：
 *   单人世界 saves/bong_weapon_test 需已存在（手动创建一次，创造模式 + 允许作弊 +
 *   超平坦推荐）；gradle runClient 通过 --quickPlaySingleplayer=bong_weapon_test
 *   直进世界。
 *
 * 状态机（ClientTick 驱动）:
 *   WAIT_WORLD → PREP → SETTLE → FP_SHOT → TP_SHOT → HOTBAR_SHOT → FINISHED(scheduleStop)
 */
public final class WeaponScreenshotHarness {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-weapon-shot");
    private static final String ENV_ITEM = "BONG_WEAPON_TEST_ITEM";
    private static final String ENV_ASSET = "BONG_WEAPON_TEST_ASSET";
    private static final String ENV_OUT = "BONG_WEAPON_TEST_OUT";
    private static final String ENV_BONG_EQUIP = "BONG_WEAPON_TEST_BONG_EQUIP";

    private WeaponScreenshotHarness() {}

    public static void install() {
        String itemId = System.getenv(ENV_ITEM);
        String assetId = System.getenv(ENV_ASSET);
        if (itemId == null || itemId.isBlank() || assetId == null || assetId.isBlank()) {
            return;
        }
        String outRoot = System.getenv(ENV_OUT);
        if (outRoot == null || outRoot.isBlank()) outRoot = "client/tools/renders";
        File outDir = new File(outRoot, assetId).getAbsoluteFile();
        if (!outDir.exists() && !outDir.mkdirs()) {
            LOGGER.error("无法创建输出目录 {}，harness 不启动", outDir);
            return;
        }
        String bongEquipTemplateId = System.getenv(ENV_BONG_EQUIP);
        if (bongEquipTemplateId != null && bongEquipTemplateId.isBlank()) bongEquipTemplateId = null;
        Impl impl = new Impl(itemId, assetId, outDir, bongEquipTemplateId);
        ClientTickEvents.END_CLIENT_TICK.register(impl::onTick);
        LOGGER.info("WeaponScreenshotHarness 启动 | item={} asset={} bong_equip={} out={}",
                itemId, assetId, bongEquipTemplateId, outDir);
    }

    private enum Phase {
        WAIT_WORLD,
        PREP,
        SETTLE,
        FP_SHOT,
        TP_BACK_SHOT,
        TP_FRONT_SHOT,
        HOTBAR_SHOT,
        FINISHED,
    }

    private static final class Impl {
        private final String itemId;
        private final String assetId;
        private final File outDir;
        /** 设了就跳过 /give，直接塞 WeaponEquippedStore 模拟 server 推送。 */
        private final String bongEquipTemplateId;

        private Phase phase = Phase.WAIT_WORLD;
        private int phaseTicks = 0;
        private boolean stopRequested = false;

        Impl(String itemId, String assetId, File outDir, String bongEquipTemplateId) {
            this.itemId = itemId;
            this.assetId = assetId;
            this.outDir = outDir;
            this.bongEquipTemplateId = bongEquipTemplateId;
        }

        void onTick(MinecraftClient client) {
            if (stopRequested) return;
            phaseTicks++;
            // WSLg 里 MC 窗口失焦会触发 pauseOnLostFocus 弹 GameMenuScreen，截图全遮；
            // 每 tick 兜底：关 pauseOnLostFocus + 清掉任何弹出 screen。
            client.options.pauseOnLostFocus = false;
            if (client.currentScreen != null && phase != Phase.WAIT_WORLD) {
                client.setScreen(null);
            }
            try {
                step(client);
            } catch (Exception e) {
                LOGGER.error("harness 崩了，退出", e);
                requestStop(client);
            }
        }

        private void step(MinecraftClient client) {
            switch (phase) {
                case WAIT_WORLD -> {
                    if (client.world != null && client.player != null
                            && client.getServer() != null && client.player.age > 5) {
                        advance(Phase.PREP);
                    } else if (phaseTicks > 20 * 45) {
                        LOGGER.error("等 45s 没进世界（saves/bong_weapon_test 存在吗？），退出");
                        requestStop(client);
                    }
                }
                case PREP -> {
                    if (phaseTicks < 20) return;
                    prepPlayer(client);
                    advance(Phase.SETTLE);
                }
                case SETTLE -> {
                    if (phaseTicks < 40) return;   // 给 /give + teleport + flight 传播
                    if (client.getToastManager() != null) client.getToastManager().clear();
                    // hudHidden=true 会**连带隐藏 FP 的 held item**（vanilla F1 行为），
                    // 所以 FP 必须 hudHidden=false；只在 TP 阶段打开。
                    client.options.hudHidden = false;
                    client.options.setPerspective(Perspective.FIRST_PERSON);
                    advance(Phase.FP_SHOT);
                }
                case FP_SHOT -> {
                    if (phaseTicks < 10) return;
                    if (client.getToastManager() != null) client.getToastManager().clear();
                    shoot(client, "mc_firstperson_righthand.png");
                    client.options.hudHidden = true;   // TP 期间关 HUD 避免杂字
                    client.options.setPerspective(Perspective.THIRD_PERSON_BACK);
                    advance(Phase.TP_BACK_SHOT);
                }
                case TP_BACK_SHOT -> {
                    if (phaseTicks < 15) return;   // 三人称 camera 切换慢一点
                    if (client.getToastManager() != null) client.getToastManager().clear();
                    shoot(client, "mc_thirdperson_back.png");
                    client.options.setPerspective(Perspective.THIRD_PERSON_FRONT);
                    advance(Phase.TP_FRONT_SHOT);
                }
                case TP_FRONT_SHOT -> {
                    if (phaseTicks < 15) return;
                    if (client.getToastManager() != null) client.getToastManager().clear();
                    shoot(client, "mc_thirdperson_front.png");
                    client.options.hudHidden = false;  // 给 hotbar 开回 HUD
                    client.options.setPerspective(Perspective.FIRST_PERSON);
                    advance(Phase.HOTBAR_SHOT);
                }
                case HOTBAR_SHOT -> {
                    // hotbar slot 0 在 HUD 底部，透明背景的 GUI transform item
                    if (phaseTicks < 10) return;
                    if (client.getToastManager() != null) client.getToastManager().clear();
                    shoot(client, "mc_hotbar.png");
                    advance(Phase.FINISHED);
                }
                case FINISHED -> {
                    if (phaseTicks < 5) return;
                    LOGGER.info("harness 完成，关闭客户端");
                    requestStop(client);
                }
            }
        }

        private void advance(Phase next) {
            phase = next;
            phaseTicks = 0;
        }

        private void requestStop(MinecraftClient client) {
            stopRequested = true;
            client.scheduleStop();
        }

        private void prepPlayer(MinecraftClient client) {
            IntegratedServer server = client.getServer();
            if (server == null) {
                LOGGER.error("无 IntegratedServer，harness 只支持单人世界");
                requestStop(client);
                return;
            }
            if (client.player == null) {
                requestStop(client);
                return;
            }
            final UUID uuid = client.player.getUuid();
            // "minecraft:air" 特殊 baseline 模式: 玩家不持任何物品(不 give),纯拍空手
            final boolean baselineMode = "minecraft:air".equals(itemId);
            final Item item = Registries.ITEM.get(Identifier.tryParse(itemId));
            if (item == Items.AIR && bongEquipTemplateId == null && !baselineMode) {
                LOGGER.error("不认识的 item id: {}", itemId);
                requestStop(client);
                return;
            }
            server.execute(() -> {
                ServerPlayerEntity sp = server.getPlayerManager().getPlayer(uuid);
                if (sp == null) {
                    LOGGER.error("server player 找不到");
                    return;
                }
                ServerWorld world = sp.getServerWorld();
                // 时间 = noon, 固定日光 + 不变天
                world.getGameRules().get(GameRules.DO_DAYLIGHT_CYCLE).set(false, server);
                world.getGameRules().get(GameRules.DO_WEATHER_CYCLE).set(false, server);
                world.setTimeOfDay(6000L);

                sp.changeGameMode(GameMode.CREATIVE);
                // creative 默认 allowFlying=true，但 flying=false 所以会坠地。手动开
                // flying + sync，玩家就能悬停在 y=120，TP 摄像机不会被地形污染。
                sp.getAbilities().flying = true;
                sp.sendAbilitiesUpdate();
                // yaw=-90 面朝 +X（正东）——这样 TP back 摄像机在 -X 方向，看玩家背面；
                // 玩家右手持剑在 +Z 一侧对着摄像头右侧视野，不被躯干挡。
                sp.networkHandler.requestTeleport(0.5, 120.0, 0.5, -90f, 0f);
                sp.getInventory().clear();

                if (baselineMode) {
                    LOGGER.info("prep (baseline): 无武器,纯玩家 body 基准帧");
                } else if (bongEquipTemplateId != null) {
                    // Bong equip 模式 (纯 Mixin 方案):
                    // vanilla PlayerInventory 保持空,client 仅往 WeaponEquippedStore 塞数据,
                    // MixinHeldItemRenderer + MixinPlayerEntityHeldItem 在 render 时合成 fake stack。
                    // 验证 plan §0 "MC 物品系统零入侵" 原则的可行性。
                    LOGGER.info("prep (bong-pure-mixin): skip server inventory insert");
                } else {
                    sp.getInventory().insertStack(new ItemStack(item));
                    sp.getInventory().selectedSlot = 0;
                    LOGGER.info("prep (vanilla): give {} @ (0.5, 120, 0.5), flying=on, yaw=-90", itemId);
                }
            });

            // Bong 模式：在 client 线程直接塞 WeaponEquippedStore 模拟 server 推送
            if (bongEquipTemplateId != null) {
                WeaponEquippedStore.putOrClear("main_hand", new EquippedWeapon(
                        "main_hand",
                        1L,
                        bongEquipTemplateId,
                        "sword",
                        200.0f,
                        200.0f,
                        0
                ));
                LOGGER.info("prep (bong): WeaponEquippedStore.main_hand = {}", bongEquipTemplateId);
            }
        }

        private void shoot(MinecraftClient client, String filename) {
            try {
                NativeImage img = ScreenshotRecorder.takeScreenshot(client.getFramebuffer());
                Path dst = outDir.toPath().resolve(filename);
                File dstFile = dst.toFile();
                img.writeTo(dstFile);
                img.close();
                LOGGER.info("截图 → {}", dst);
            } catch (IOException e) {
                LOGGER.error("写截图失败 {}", filename, e);
            }
        }
    }
}
