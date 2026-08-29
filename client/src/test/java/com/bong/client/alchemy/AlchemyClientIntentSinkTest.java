package com.bong.client.alchemy;

import com.bong.client.alchemy.state.RecipeScrollStore;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyClientIntentSinkTest {
    private final RecordingTransport transport = new RecordingTransport();
    private final AlchemyClientIntentSink sink = new AlchemyClientIntentSink(transport);
    private static final BlockPos POS = new BlockPos(1, 64, 2);

    @BeforeEach
    void setUp() {
        RecipeScrollStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        RecipeScrollStore.resetForTests();
    }

    @Test
    void validatesNullBoundaryAndEveryNumericDomain() {
        assertEquals("LOCAL_REJECTED", sink.dispatch(null).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.TurnPage(0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.FeedSlot(POS, 4, "herb", 1)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.FeedSlot(null, 0, "herb", 1)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.FeedSlot(POS, 0, "", 1)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.FeedSlot(POS, 0, "herb", 0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.InjectQi(POS, 0.0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.InjectQi(POS, Double.NaN)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.AdjustTemp(POS, 1.01)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.Ignite(POS, " ")).kind().name());
    }

    @Test
    void dispatchesEveryValidActionWithoutExposingSenderToScreen() {
        assertTrue(sink.dispatch(new AlchemyIntent.TurnPage(-1)).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.LearnRecipe("new_recipe")).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.FeedSlot(POS, 2, "spirit_grass", 3)).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.TakeBack(POS, 2)).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.InjectQi(POS, 1.5)).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.Ignite(POS, "kai_mai_pill_v0")).kind().name().endsWith("ACCEPTED"));
        assertTrue(sink.dispatch(new AlchemyIntent.AdjustTemp(POS, 0.62)).kind().name().endsWith("ACCEPTED"));
        assertEquals(7, transport.calls);
        assertEquals("adjust:0.62", transport.lastCall);
    }

    @Test
    void duplicateLocalRecipeAndTransportFailureAreObservable() {
        assertEquals("LOCAL_ACCEPTED", sink.dispatch(new AlchemyIntent.LearnRecipe("new_recipe")).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new AlchemyIntent.LearnRecipe("new_recipe")).kind().name());
        transport.fail = true;
        assertEquals("LOCAL_ERROR", sink.dispatch(new AlchemyIntent.TurnPage(1)).kind().name());
    }

    private static final class RecordingTransport implements AlchemyClientIntentSink.Transport {
        private int calls;
        private String lastCall = "";
        private boolean fail;

        private void record(String call) {
            if (fail) throw new IllegalStateException("offline");
            calls++;
            lastCall = call;
        }

        @Override public void turnPage(int delta) { record("turn:" + delta); }
        @Override public void learnRecipe(String recipeId) { record("learn:" + recipeId); }
        @Override public void feedSlot(BlockPos pos, int slot, String material, int count) { record("feed:" + slot); }
        @Override public void takeBack(BlockPos pos, int slot) { record("take:" + slot); }
        @Override public void injectQi(BlockPos pos, double amount) { record("qi:" + amount); }
        @Override public void ignite(BlockPos pos, String recipeId) { record("ignite:" + recipeId); }
        @Override public void adjustTemp(BlockPos pos, double temperature) { record("adjust:" + temperature); }
    }
}
