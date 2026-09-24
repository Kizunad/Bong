package com.bong.client.alchemy;

import com.bong.client.ui.contract.UiIntent;
import net.minecraft.util.math.BlockPos;

/** 炼丹屏允许的 typed actions；UI 不再直接调用 C2S sender。 */
public sealed interface AlchemyIntent extends UiIntent permits
    AlchemyIntent.TurnPage,
    AlchemyIntent.LearnRecipe,
    AlchemyIntent.FeedSlot,
    AlchemyIntent.TakeBack,
    AlchemyIntent.InjectQi,
    AlchemyIntent.Ignite,
    AlchemyIntent.AdjustTemp {

    record TurnPage(int delta) implements AlchemyIntent {}

    record LearnRecipe(String recipeId) implements AlchemyIntent {}

    record FeedSlot(BlockPos furnacePos, int slot, String material, int count) implements AlchemyIntent {}

    record TakeBack(BlockPos furnacePos, int slot) implements AlchemyIntent {}

    record InjectQi(BlockPos furnacePos, double amount) implements AlchemyIntent {}

    record Ignite(BlockPos furnacePos, String recipeId) implements AlchemyIntent {}

    record AdjustTemp(BlockPos furnacePos, double temperature) implements AlchemyIntent {}
}
