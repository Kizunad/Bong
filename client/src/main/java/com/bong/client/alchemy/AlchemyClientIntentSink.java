package com.bong.client.alchemy;

import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/** 将炼丹 typed action 映射到既有 sender；返回值只表示本地 transport。 */
public final class AlchemyClientIntentSink implements UiIntentSink<AlchemyIntent> {
    private static final int FURNACE_SLOTS = 4;
    private final Transport transport;

    AlchemyClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static AlchemyClientIntentSink production() {
        return new AlchemyClientIntentSink(new Transport() {
            @Override public void turnPage(int delta) { ClientRequestSender.sendAlchemyTurnPage(delta); }
            @Override public void learnRecipe(String recipeId) { ClientRequestSender.sendAlchemyLearnRecipe(recipeId); }
            @Override public void feedSlot(BlockPos pos, int slot, String material, int count) {
                ClientRequestSender.sendAlchemyFeedSlot(pos, slot, material, count);
            }
            @Override public void takeBack(BlockPos pos, int slot) { ClientRequestSender.sendAlchemyTakeBack(pos, slot); }
            @Override public void injectQi(BlockPos pos, double amount) { ClientRequestSender.sendAlchemyInjectQi(pos, amount); }
            @Override public void ignite(BlockPos pos, String recipeId) { ClientRequestSender.sendAlchemyIgnite(pos, recipeId); }
            @Override public void adjustTemp(BlockPos pos, double temperature) {
                ClientRequestSender.sendAlchemyAdjustTemp(pos, temperature);
            }
        });
    }

    @Override
    public UiIntentResult dispatch(AlchemyIntent intent) {
        if (intent == null) return UiIntentResult.rejected("alchemy intent must not be null");
        try {
            if (intent instanceof AlchemyIntent.TurnPage page) {
                if (page.delta() == 0) return UiIntentResult.rejected("page delta must not be zero");
                try {
                    transport.turnPage(page.delta());
                    return UiIntentResult.accepted();
                } catch (RuntimeException failure) {
                    // 没有 server transport 时只更新本地翻页镜像，并把失败明确交给 UI。
                    RecipeScrollStore.turn(page.delta());
                    return transportError(failure);
                }
            }
            if (intent instanceof AlchemyIntent.LearnRecipe learn) {
                String id = required(learn.recipeId(), "recipe id");
                boolean learned = RecipeScrollStore.learn(new RecipeScrollStore.RecipeEntry(
                    id, id, "§7新悟得方子: " + id
                ));
                if (!learned) return UiIntentResult.rejected("recipe already learned");
                // 保留旧屏幕的本地悟方语义：先更新本地镜像，再尝试通知服务端；
                // transport 失败仍明确返回 LOCAL_ERROR，不把它伪装成 server 成功。
                transport.learnRecipe(id);
                return UiIntentResult.accepted();
            }
            if (intent instanceof AlchemyIntent.FeedSlot feed) {
                requireSlot(feed.slot());
                requirePos(feed.furnacePos());
                transport.feedSlot(feed.furnacePos(), feed.slot(), required(feed.material(), "material"), requirePositive(feed.count(), "count"));
                return UiIntentResult.accepted();
            }
            if (intent instanceof AlchemyIntent.TakeBack takeBack) {
                requireSlot(takeBack.slot());
                requirePos(takeBack.furnacePos());
                transport.takeBack(takeBack.furnacePos(), takeBack.slot());
                return UiIntentResult.accepted();
            }
            if (intent instanceof AlchemyIntent.InjectQi inject) {
                requirePos(inject.furnacePos());
                if (!Double.isFinite(inject.amount()) || inject.amount() <= 0.0) {
                    return UiIntentResult.rejected("qi amount must be finite and > 0");
                }
                transport.injectQi(inject.furnacePos(), inject.amount());
                return UiIntentResult.accepted();
            }
            if (intent instanceof AlchemyIntent.Ignite ignite) {
                requirePos(ignite.furnacePos());
                transport.ignite(ignite.furnacePos(), required(ignite.recipeId(), "recipe id"));
                return UiIntentResult.accepted();
            }
            AlchemyIntent.AdjustTemp adjust = (AlchemyIntent.AdjustTemp) intent;
            requirePos(adjust.furnacePos());
            if (!Double.isFinite(adjust.temperature()) || adjust.temperature() < 0.0 || adjust.temperature() > 1.0) {
                return UiIntentResult.rejected("temperature must be within [0, 1]");
            }
            transport.adjustTemp(adjust.furnacePos(), adjust.temperature());
            return UiIntentResult.accepted();
        } catch (IllegalArgumentException failure) {
            return UiIntentResult.rejected(failure.getMessage());
        } catch (RuntimeException failure) {
            return transportError(failure);
        }
    }

    private static UiIntentResult transportError(RuntimeException failure) {
        String detail = failure.getMessage();
        return UiIntentResult.error("alchemy transport failed: "
            + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
    }

    private static String required(String value, String name) {
        if (value == null || value.isBlank()) throw new IllegalArgumentException(name + " must not be blank");
        return value.strip();
    }

    private static int requirePositive(int value, String name) {
        if (value <= 0) throw new IllegalArgumentException(name + " must be > 0");
        return value;
    }

    private static void requirePos(BlockPos pos) {
        if (pos == null) throw new IllegalArgumentException("furnace position must not be null");
    }

    private static void requireSlot(int slot) {
        if (slot < 0 || slot >= FURNACE_SLOTS) throw new IllegalArgumentException("slot must be within [0, 3]");
    }

    interface Transport {
        void turnPage(int delta);
        void learnRecipe(String recipeId);
        void feedSlot(BlockPos pos, int slot, String material, int count);
        void takeBack(BlockPos pos, int slot);
        void injectQi(BlockPos pos, double amount);
        void ignite(BlockPos pos, String recipeId);
        void adjustTemp(BlockPos pos, double temperature);
    }
}
