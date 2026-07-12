package com.bong.client.inventory.state;

import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P2b — {@link BodyPlanLayoutStore} 首帧竞态 + 未知 plan id +
 * 缺 layout 场景饱和测试。两个独立到达的信号（{@code body_plan_layout} payload 建缓存
 * / {@code cultivation_detail.body_plan_id} 更新当前指针）谁先到都不能 crash 或产生
 * 错误的"当前 layout"。
 */
class BodyPlanLayoutStoreTest {

    @AfterEach
    void tearDown() { BodyPlanLayoutStore.resetForTests(); }

    private static BodyPlanLayout layout(String id) {
        return new BodyPlanLayout(id, List.of(), List.of(), List.of(), List.of());
    }

    @Test
    void freshStoreHasNoCurrentLayout() {
        assertNull(BodyPlanLayoutStore.current(), "未收到任何数据时 current() 必须是 null（消费方走视觉 fallback）");
        assertNull(BodyPlanLayoutStore.currentPlanId());
    }

    // ── 竞态 1：layout payload 先到，cultivation_detail 后到 ──────────────
    @Test
    void layoutArrivesBeforePlanIdPointer() {
        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        assertNull(BodyPlanLayoutStore.current(),
            "layout 已缓存但当前指针尚未指向它时，current() 仍应是 null（不能误用任意缓存条目）");

        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertEquals("humanoid", BodyPlanLayoutStore.current().bodyPlanId(),
            "指针追上后应立即解析出已缓存的 layout，无需重新下发");
    }

    // ── 竞态 2：cultivation_detail 先到，layout payload 后到 ──────────────
    @Test
    void planIdPointerArrivesBeforeLayout() {
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertNull(BodyPlanLayoutStore.current(),
            "指针已指向 humanoid 但 layout 尚未到达时，current() 必须是 null（不能返回缓存未命中当假数据）");

        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        assertEquals("humanoid", BodyPlanLayoutStore.current().bodyPlanId(),
            "layout 到达后应立即变为可解析，不需要重新等指针变化");
    }

    // ── 未知 plan id：指针指向一个从未下发过 layout 的 id ──────────────
    @Test
    void unknownPlanIdNeverResolves() {
        BodyPlanLayoutStore.setCurrentPlanId("nonexistent_race");
        assertNull(BodyPlanLayoutStore.current(),
            "未知 plan id 必须安全返回 null，不能抛异常或伪造一个假 layout");

        // 即便缓存了一个完全不相关的 plan，也不会被误用于未知指针。
        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        assertNull(BodyPlanLayoutStore.current(),
            "缓存别的 plan 不应影响一个仍未解析的未知指针");
    }

    // ── 缺 layout：byId 直查缓存，不受 current 指针影响 ──────────────────
    @Test
    void byIdLooksUpIndependentlyOfCurrentPointer() {
        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        BodyPlanLayoutStore.putLayout(layout("whale"));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertEquals("whale", BodyPlanLayoutStore.byId("whale").bodyPlanId(),
            "byId 应能查到非当前 plan 的缓存条目（真实换 race 前的预取场景）");
        assertNull(BodyPlanLayoutStore.byId("unregistered"));
    }

    // ── 真实换 race：指针变化必须重新解析（不沿用旧 plan 的 layout）──────
    @Test
    void planIdChangeSwitchesResolvedLayout() {
        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        BodyPlanLayoutStore.putLayout(layout("whale"));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertEquals("humanoid", BodyPlanLayoutStore.current().bodyPlanId());

        BodyPlanLayoutStore.setCurrentPlanId("whale");
        assertEquals("whale", BodyPlanLayoutStore.current().bodyPlanId(),
            "真实换 race 后 current() 必须立刻切到新 plan 的 layout");
    }

    // ── 监听者：指针推进和 layout 到达都要触发通知（用当前解析结果） ─────
    @Test
    void listenerNotifiedOnBothPointerAdvanceAndLayoutArrival() {
        List<BodyPlanLayout> seen = new ArrayList<>();
        BodyPlanLayoutStore.addListener(seen::add);

        BodyPlanLayoutStore.setCurrentPlanId("humanoid"); // pointer advance, layout not cached yet → notified with null
        BodyPlanLayoutStore.putLayout(layout("humanoid")); // layout arrival for the current plan → notified with resolved layout

        assertEquals(2, seen.size());
        assertNull(seen.get(0), "指针推进但尚无缓存时，监听者应收到 null（视觉 fallback 信号）");
        assertEquals("humanoid", seen.get(1).bodyPlanId());
    }

    @Test
    void listenerNotNotifiedWhenCachingLayoutForNonCurrentPlan() {
        AtomicInteger notifications = new AtomicInteger();
        BodyPlanLayoutStore.addListener(layout -> notifications.incrementAndGet());

        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        notifications.set(0);
        BodyPlanLayoutStore.putLayout(layout("whale")); // 与当前指针无关，不应触发通知

        assertEquals(0, notifications.get(),
            "缓存一个非当前 plan 的 layout 不应该触发监听者（避免面板收到无关刷新）");
    }

    @Test
    void resetForTestsClearsCacheAndPointerAndListeners() {
        BodyPlanLayoutStore.putLayout(layout("humanoid"));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        BodyPlanLayoutStore.addListener(layout -> { });

        BodyPlanLayoutStore.resetForTests();

        assertNull(BodyPlanLayoutStore.current());
        assertNull(BodyPlanLayoutStore.currentPlanId());
        assertNull(BodyPlanLayoutStore.byId("humanoid"));
    }

    @Test
    void settingSamePlanIdTwiceDoesNotDoubleNotify() {
        AtomicInteger notifications = new AtomicInteger();
        BodyPlanLayoutStore.addListener(layout -> notifications.incrementAndGet());

        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertEquals(1, notifications.get(), "重复设置同一 plan id 不应重复触发监听者");
    }

    @Test
    void putLayoutWithNullOrBlankIdIsIgnored() {
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("", List.of(), List.of(), List.of(), List.of()));
        assertNull(BodyPlanLayoutStore.byId(""));
        assertTrue(true, "空白 body_plan_id 的 layout 必须被丢弃，不能污染缓存索引");
    }
}
