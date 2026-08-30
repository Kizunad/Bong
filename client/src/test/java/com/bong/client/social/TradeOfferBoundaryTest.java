package com.bong.client.social;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TradeOfferBoundaryTest {
    @AfterEach
    void resetStore() {
        SocialStateStore.resetForTests();
    }

    @Test
    void choicesAreStableAndRetainExactInstanceIdentityAcrossGridAndHotbar() {
        InventoryItem z = InventoryItem.createFull(91L, "z", "Zeta", 1, 1, 1, "common", "", 1, 1, 1);
        InventoryItem a = InventoryItem.createFull(42L, "a", "Alpha", 1, 1, 1, "common", "", 1, 1, 1);
        InventoryModel model = InventoryModel.builder()
            .gridItem(z, InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .hotbar(0, a)
            .build();
        List<InventoryItem> choices = TradeOfferScreenViewModel.collectChoices(model);
        assertEquals(List.of(42L, 91L), choices.stream().map(InventoryItem::instanceId).toList());
        assertTrue(TradeOfferScreenViewModel.findChoice(model, 91L).isPresent(),
            "显式 instance_id 必须能解析到对应物品");
        assertTrue(TradeOfferScreenViewModel.findChoice(model, 1L).isEmpty(),
            "不存在的 instance_id 不得被 picker 接受");
    }

    @Test
    void selectionNeverFallsBackToAnotherItemWhenTheExactIdentityDisappears() {
        InventoryItem first = InventoryItem.createFull(42L, "a", "Alpha", 1, 1, 1, "common", "", 1, 1, 1);
        InventoryItem second = InventoryItem.createFull(91L, "z", "Zeta", 1, 1, 1, "common", "", 1, 1, 1);
        TradeOfferPicker picker = new TradeOfferPicker(List.of(first, second));
        assertEquals(-1, picker.selectedIndex(),
            "交易屏初始必须保持无选择，不能自动选第一件");
        assertTrue(picker.move(1), "显式下一件操作应产生选择");
        assertEquals(42L, picker.selectedInstanceId());
        assertTrue(picker.move(1), "再次移动应切换到下一件");
        assertEquals(91L, picker.selectedInstanceId());
        picker.update(List.of(second));
        assertEquals(0, picker.selectedIndex(), "库存更新时必须保留原 exact instance_id");
        picker.update(List.of(first));
        assertEquals(-1, picker.selectedIndex(),
            "exact instance_id 消失后必须回到无选择，不能静默改选其他物品");
        assertTrue(picker.selectedFrom(List.of(second)).isEmpty(),
            "提交时必须在 authoritative 快照中重新确认 exact instance_id");
    }

    @Test
    void pickerSupportsExplicitPreviousAndRejectsEmptyOrZeroMovement() {
        InventoryItem first = InventoryItem.createFull(42L, "a", "Alpha", 1, 1, 1, "common", "", 1, 1, 1);
        InventoryItem second = InventoryItem.createFull(91L, "z", "Zeta", 1, 1, 1, "common", "", 1, 1, 1);
        TradeOfferPicker picker = new TradeOfferPicker(List.of(first, second));
        assertFalse(picker.move(0), "零步长不能偷偷产生选择");
        assertTrue(picker.move(-1), "显式上一件应从最后一件开始");
        assertEquals(91L, picker.selectedInstanceId());
        picker.update(List.of());
        assertFalse(picker.move(1), "没有可交换物品时不能移动选择");
        assertTrue(picker.selectedFrom(List.of(first, second)).isEmpty(),
            "没有选择时不能提交任意物品");
    }

    @Test
    void responseSinkRejectsMissingSelectionAndPreservesDeclineShape() {
        RecordingTransport transport = new RecordingTransport();
        TradeOfferClientIntentSink sink = new TradeOfferClientIntentSink(transport);
        assertEquals("LOCAL_REJECTED", sink.dispatch(new TradeOfferIntent.Respond("offer", true, null)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new TradeOfferIntent.Respond("offer", false, 42L)).kind().name());
        assertEquals("LOCAL_ACCEPTED", sink.dispatch(new TradeOfferIntent.Respond("offer", true, 42L)).kind().name());
        assertEquals("offer:true:42", transport.call);
        assertTrue(sink.dispatch(new TradeOfferIntent.Respond("offer", false, null)).kind().name().endsWith("ACCEPTED"));
        assertEquals("offer:false:null", transport.call);
    }

    @Test
    void successfulResponseClearsOnlyTheMatchingLocalOffer() {
        SocialStateStore.replaceTradeOffer(new SocialStateStore.TradeOffer(
            "offer-1", "alice", "bob",
            new SocialStateStore.TradeItemSummary(7L, "herb", "灵草", 1),
            List.of(), System.currentTimeMillis() + 10_000L
        ));
        RecordingTransport transport = new RecordingTransport();
        TradeOfferClientIntentSink sink = new TradeOfferClientIntentSink(transport);

        assertEquals("LOCAL_ACCEPTED", sink.dispatch(
            new TradeOfferIntent.Respond("offer-1", false, null)
        ).kind().name());
        assertNull(SocialStateStore.tradeOffer(),
            "response 成功后必须清理同一 offer，避免关闭屏幕后下一个 tick 重开旧邀请");
    }

    private static final class RecordingTransport implements TradeOfferClientIntentSink.Transport {
        private String call;

        @Override
        public void respond(String offerId, boolean accepted, Long requestedInstanceId) {
            call = offerId + ":" + accepted + ":" + requestedInstanceId;
        }
    }
}
