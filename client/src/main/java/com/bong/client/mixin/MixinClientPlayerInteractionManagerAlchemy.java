package com.bong.client.mixin;

import com.bong.client.alchemy.AlchemyFurnaceItems;
import com.bong.client.alchemy.AlchemyFurnaceInteractionRules;
import com.bong.client.alchemy.AlchemyScreenBootstrap;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.coffin.CoffinEnterIntentHandler;
import com.bong.client.combat.screen.ZhenfaLayoutScreen;
import com.bong.client.entity.BongEntityModelKind;
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
import net.minecraft.util.math.Direction;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(ClientPlayerInteractionManager.class)
public abstract class MixinClientPlayerInteractionManagerAlchemy {
    private static final String MUNDANE_COFFIN_ITEM_ID = "mundane_coffin";
    private static final String WARNING_TRAP_ITEM_ID = "warning_trap";
    private static final String BLAST_TRAP_ITEM_ID = "blast_trap";
    private static final String SLOW_TRAP_ITEM_ID = "slow_trap";

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
        ClientRequestProtocol.ZhenfaKind zhenfaTrapKind = bong$zhenfaKindForItem(mainHand);
        if (zhenfaTrapKind != null && mainHand.instanceId() > 0) {
            client.setScreen(new ZhenfaLayoutScreen(
                hit.getBlockPos(),
                zhenfaTrapKind,
                mainHand.instanceId(),
                bong$zhenfaFace(hit.getSide())
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

    private static boolean bong$isSpawnTutorialCoffin(MinecraftClient client, BlockPos pos) {
        if (!client.world.getBlockState(pos).isOf(Blocks.CHISELED_STONE_BRICKS)) {
            return false;
        }
        return Math.abs(pos.getX()) <= 8 && pos.getY() >= 60 && pos.getY() <= 90 && Math.abs(pos.getZ()) <= 8;
    }

    private static ClientRequestProtocol.ZhenfaKind bong$zhenfaKindForItem(InventoryItem item) {
        if (item == null) return null;
        return switch (item.itemId()) {
            case WARNING_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.WARNING_TRAP;
            case BLAST_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.BLAST_TRAP;
            case SLOW_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.SLOW_TRAP;
            default -> null;
        };
    }

    private static ClientRequestProtocol.ZhenfaTargetFace bong$zhenfaFace(Direction direction) {
        return switch (direction) {
            case UP -> ClientRequestProtocol.ZhenfaTargetFace.TOP;
            case DOWN -> ClientRequestProtocol.ZhenfaTargetFace.BOTTOM;
            case NORTH -> ClientRequestProtocol.ZhenfaTargetFace.NORTH;
            case SOUTH -> ClientRequestProtocol.ZhenfaTargetFace.SOUTH;
            case EAST -> ClientRequestProtocol.ZhenfaTargetFace.EAST;
            case WEST -> ClientRequestProtocol.ZhenfaTargetFace.WEST;
        };
    }
}
