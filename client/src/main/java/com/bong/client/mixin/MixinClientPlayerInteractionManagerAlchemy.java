package com.bong.client.mixin;

import com.bong.client.alchemy.AlchemyFurnaceItems;
import com.bong.client.alchemy.AlchemyFurnaceInteractionRules;
import com.bong.client.alchemy.AlchemyScreenBootstrap;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.block.BlockPlaceIntentResolver;
import com.bong.client.coffin.CoffinEnterIntentHandler;
import com.bong.client.coffin.TutorialCoffinPosRules;
import com.bong.client.coffin.TutorialCoffinPosStore;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.craft.WorkbenchPlaceDust;
import com.bong.client.combat.screen.ZhenfaLayoutScreen;
import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.interaction.ClientInteractionItemResolver;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.hud.TargetInfoStateStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.block.Blocks;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.network.ClientPlayerInteractionManager;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.ActionResult;
import net.minecraft.util.Hand;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.math.BlockPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(ClientPlayerInteractionManager.class)
public abstract class MixinClientPlayerInteractionManagerAlchemy {
    private static final String MUNDANE_COFFIN_ITEM_ID = "mundane_coffin";

    @Inject(method = "attackEntity", at = @At("TAIL"))
    @SuppressWarnings({"unused", "PMD.UnusedPrivateMethod"})
    private void bong$targetInfoAttack(PlayerEntity player, Entity target, CallbackInfo ci) {
        TargetInfoStateStore.observeEntity(target, System.currentTimeMillis());
        // plan-coffin-tiers-v1 P3 — 左键攻击延寿棺 marker 实体 → coffin_break C2S。
        // marker 实体无服务端碰撞箱，MC 客户端 attackEntity 照常触发；
        // 在此截获并发 coffin_break，体验等同破坏方块。
        if (target instanceof BongModeledEntity modeled
            && CoffinEnterIntentHandler.isCoffinKind(modeled.modelKind())) {
            // getBlockPos() = floor(entity.x, y, z)，对应 marker 坐标所在格
            // （marker 位于 lower.x+1 即 upper half），server registry 按 lower/upper 归一。
            ClientRequestSender.sendCoffinBreak(target.getBlockPos());
        }
    }

    @Inject(method = "interactEntity", at = @At("TAIL"))
    @SuppressWarnings({"unused", "PMD.UnusedPrivateMethod"})
    private void bong$targetInfoInteract(
        PlayerEntity player,
        Entity entity,
        Hand hand,
        CallbackInfoReturnable<ActionResult> cir
    ) {
        if (hand == Hand.MAIN_HAND) {
            TargetInfoStateStore.observeEntity(entity, System.currentTimeMillis());
        }
    }

    @Inject(method = "interactBlock", at = @At("HEAD"), cancellable = true)
    private void bong$alchemyInteractBlock(
        ClientPlayerEntity player,
        Hand hand,
        BlockHitResult hit,
        CallbackInfoReturnable<ActionResult> cir
    ) {
        if (hand != Hand.MAIN_HAND || player == null || hit == null) return;

        MinecraftClient client = MinecraftClient.getInstance();
        if (client.world != null) {
            BlockPos pos = hit.getBlockPos();
            if (bong$isSpawnTutorialCoffin(client, pos)) {
                ClientRequestSender.sendCoffinOpen(pos);
                cir.setReturnValue(ActionResult.SUCCESS);
                return;
            }
        }

        InventoryItem mainHand = InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND);
        Long scatterBeadInstanceId = ClientInteractionItemResolver.qiScatterBeadUseInstanceId(mainHand);
        if (scatterBeadInstanceId != null) {
            if (player.isSneaking()) {
                ClientRequestSender.sendQiScatterBeadUse(
                    scatterBeadInstanceId,
                    hit.getBlockPos().offset(hit.getSide())
                );
            } else {
                ClientRequestSender.sendQiScatterBeadUse(scatterBeadInstanceId);
            }
            player.swingHand(Hand.MAIN_HAND);
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        ClientRequestProtocol.ZhenfaKind zhenfaTrapKind = ClientInteractionItemResolver.zhenfaKindForItem(mainHand);
        if (zhenfaTrapKind != null && mainHand.instanceId() > 0) {
            client.setScreen(new ZhenfaLayoutScreen(
                hit.getBlockPos(),
                zhenfaTrapKind,
                mainHand.instanceId(),
                BlockPlaceIntentResolver.zhenfaFace(hit.getSide())
            ));
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        if (mainHand != null
            && MUNDANE_COFFIN_ITEM_ID.equals(mainHand.itemId())
            && mainHand.instanceId() > 0) {
            BlockPos placePos = hit.getBlockPos().offset(hit.getSide());
            ClientRequestSender.sendCoffinPlace(placePos, mainHand.instanceId());
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        if (mainHand != null
            && AlchemyFurnaceItems.isFurnaceItem(mainHand.itemId())
            && mainHand.instanceId() > 0) {
            BlockPos placePos = hit.getBlockPos().offset(hit.getSide());
            ClientRequestSender.sendAlchemyFurnacePlace(placePos, mainHand.instanceId());
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        InventoryItem selectedBlockItem = BlockPlaceIntentResolver.selectedBlockItem(
            SkillBarStore.selectedSlot(),
            InventoryStateStore.snapshot()
        );
        BlockPlaceIntentResolver.Intent blockPlace = BlockPlaceIntentResolver.selectedBlockPlaceIntent(
            SkillBarStore.selectedSlot(),
            InventoryStateStore.snapshot(),
            hit.getBlockPos(),
            hit.getSide()
        );
        if (blockPlace != null) {
            ClientRequestSender.sendBlockPlace(blockPlace.placePos(), blockPlace.instanceId(), blockPlace.face());
            if (selectedBlockItem != null && WorkbenchPlaceDust.shouldSpawnForItem(selectedBlockItem.itemId())) {
                WorkbenchPlaceDust.spawn(client, blockPlace.placePos());
            }
            player.swingHand(Hand.MAIN_HAND);
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        // plan-coffin-tiers-v1 P3 — CHEST→coffin_enter 旧路径已退役。
        // P2 起延寿棺改为 marker 实体（坐标 AIR，无 CHEST 方块），进棺统一由
        // CoffinEnterIntentHandler 触发（右键/G → CoffinMenuScreen → [入眠]）。
        if (client.world == null) return;
        BlockPos pos = hit.getBlockPos();
        if (client.world.getBlockState(pos).isOf(Blocks.FURNACE)
            && AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace(pos, AlchemyFurnaceStore.snapshot())) {
            AlchemyScreenBootstrap.requestOpenAlchemyScreen(client, pos);
            cir.setReturnValue(ActionResult.SUCCESS);
        }
    }

    // F9 跨层修复：出生引导棺 client 判定曾硬编码 |x|<=8, y∈[60,90], |z|<=8 —— spawn
    // 区域随地形重生成迁移到该盒外时，client 永远判负，右键真棺不发 coffin_open，
    // 棺静默打不开（module-map triage 实证的跨层孤岛）。
    //
    // 现在改用 server join 时广播的 TutorialCoffin 权威坐标（server/src/world/
    // spawn_tutorial.rs 的 TutorialCoffin.pos，见 TutorialCoffinPosHandler /
    // TutorialCoffinPosStore）做精确比对；server 侧 handle_coffin_open_requests 本就
    // 按精确 pos 匹配 + COFFIN_OPEN_INTERACT_RADIUS 校验，不依赖 client 的坐标猜测。
    //
    // fallback 抉择：尚未收到广播时返回 false（fail-closed），不再退回旧硬编码盒。
    // 理由——① server join 时几乎立即广播该坐标，未收到的窗口只有一两个 tick；
    // ② 继续用旧盒 fallback 会重新引入本次要修的那一类回归（spawn 迁移后盒失配）；
    // ③ 误判为"非引导棺"最坏后果只是这一次右键不触发（server 侧本就会因坐标不匹配
    // 拒绝 coffin_open），不会破坏任何已持有状态，玩家挪动/重试即可恢复。
    private static boolean bong$isSpawnTutorialCoffin(MinecraftClient client, BlockPos pos) {
        if (!client.world.getBlockState(pos).isOf(Blocks.CHISELED_STONE_BRICKS)) {
            return false;
        }
        return TutorialCoffinPosRules.isSpawnTutorialCoffinPos(TutorialCoffinPosStore.snapshot(), pos);
    }
}