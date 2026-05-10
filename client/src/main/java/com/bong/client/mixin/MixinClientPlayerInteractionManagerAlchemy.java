package com.bong.client.mixin;

import com.bong.client.alchemy.AlchemyFurnaceItems;
import com.bong.client.alchemy.AlchemyFurnaceInteractionRules;
import com.bong.client.alchemy.AlchemyScreen;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.hud.TargetInfoStateStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
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
    @Inject(method = "attackEntity", at = @At("TAIL"))
    private void bong$targetInfoAttack(PlayerEntity player, Entity target, CallbackInfo ci) {
        TargetInfoStateStore.observeEntity(target, System.currentTimeMillis());
    }

    @Inject(method = "interactEntity", at = @At("TAIL"))
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
        if (mainHand != null
            && AlchemyFurnaceItems.isFurnaceItem(mainHand.itemId())
            && mainHand.instanceId() > 0) {
            BlockPos placePos = hit.getBlockPos().offset(hit.getSide());
            ClientRequestSender.sendAlchemyFurnacePlace(placePos, mainHand.instanceId());
            cir.setReturnValue(ActionResult.SUCCESS);
            return;
        }

        if (client.world == null) return;
        BlockPos pos = hit.getBlockPos();
        if (client.world.getBlockState(pos).isOf(Blocks.FURNACE)
            && AlchemyFurnaceInteractionRules.shouldOpenAlchemyFurnace(pos, AlchemyFurnaceStore.snapshot())) {
            ClientRequestSender.sendAlchemyOpenFurnace(pos);
            client.execute(() -> client.setScreen(new AlchemyScreen(pos)));
            cir.setReturnValue(ActionResult.SUCCESS);
        }
    }

    private static boolean bong$isSpawnTutorialCoffin(MinecraftClient client, BlockPos pos) {
        if (!client.world.getBlockState(pos).isOf(Blocks.CHISELED_STONE_BRICKS)) {
            return false;
        }
        return Math.abs(pos.getX()) <= 8 && pos.getY() >= 60 && pos.getY() <= 90 && Math.abs(pos.getZ()) <= 8;
    }
}
