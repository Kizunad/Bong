package com.bong.client;

import com.bong.client.animation.BongAnimationPlayer;
import com.bong.client.animation.BongAnimations;
import com.bong.client.animation.BongPunchCombo;
import com.bong.client.botany.BotanyHudBootstrap;
import com.bong.client.combat.CombatHudBootstrap;
import com.bong.client.debug.BongAnimCommand;
import com.bong.client.debug.BongSpawnParticleCommand;
import com.bong.client.debug.BongVfxCommand;
import com.bong.client.insight.ClientRequestInsightDispatcher;
import com.bong.client.insight.InsightOfferScreenBootstrap;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.alchemy.AlchemyScreenBootstrap;
import com.bong.client.inventory.DroppedItemPickupBootstrap;
import com.bong.client.inventory.InspectScreenBootstrap;
import com.bong.client.lingtian.LingtianActionScreenBootstrap;
import com.bong.client.tsy.ExtractInteractionBootstrap;
import com.bong.client.ui.CultivationScreenBootstrap;
import com.bong.client.visual.particle.BongParticles;
import com.bong.client.visual.particle.VfxBootstrap;
import com.bong.client.weapon.WeaponRenderBootstrap;
import com.bong.client.weapon.WeaponScreenshotHarness;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class BongClient implements ClientModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("bong-client");

    @Override
    public void onInitializeClient() {
        LOGGER.info("Initializing Bong Client...");

        BongNetworkHandler.register();
        HudRenderCallback.EVENT.register(BongHud::render);
        CultivationScreenBootstrap.register();
        InspectScreenBootstrap.register();
        DroppedItemPickupBootstrap.register();
        com.bong.client.inventory.render.DroppedItemWorldRenderer.register();
        AlchemyScreenBootstrap.register();
        LingtianActionScreenBootstrap.register();
        InsightOfferScreenBootstrap.register();
        InsightOfferStore.setDispatcher(new ClientRequestInsightDispatcher());
        BongVfxCommand.register();
        // 粒子事件通过 client.execute 派发到主线程（BongNetworkHandler 里），在第一次 tick 之前
        // 注册完 VfxRegistry 即可；放在这里不依赖 channel register 的时序。
        BongParticles.register();
        BongParticles.registerClient();
        VfxBootstrap.registerDefaults();
        BongAnimations.bootstrap();
        BongAnimationPlayer.init();
        BongPunchCombo.bootstrap();
        BongAnimCommand.register();
        BongSpawnParticleCommand.register();
        CombatHudBootstrap.register();
        BotanyHudBootstrap.register();
        ExtractInteractionBootstrap.register();
        WeaponRenderBootstrap.register();
        WeaponScreenshotHarness.install();

        LOGGER.info("Bong Client bootstrap ready: network, HUD, keybinding scheduler, /vfx /anim /spawnp commands active.");
    }
}
