package com.bong.client.inventory.component;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.inspect.StatusPanelExtension;
import com.bong.client.combat.StatusEffectIcons;
import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tarkov 背包检视界面 buff 条的纯逻辑契约：布局收起/展开、排序、命中测试、
 * 剩余时间格式化、tooltip 文案拼接。draw()/drawTooltip() 本体依赖
 * {@code MinecraftClient.getInstance().textRenderer}，单测环境无 MC 启动会 NPE，
 * 因此这里只覆盖抽出的纯函数（与仓库既有 ItemTooltipPanelTest/StatusEffectHudPlannerTest
 * 同一约束——见各自类头注释）。
 */
class BuffBarPanelTest {
    @AfterEach void cleanup() { StatusEffectStore.clear(); }

    @Test
    void indefiniteBleedingSnapshotKeepsItsSourceIconAndNoNumericCountdown() {
        String json = """
            {"v":1,"type":"status_snapshot","effects":[
              {"id":"bleeding","name":"流血","kind":"dot","stacks":1,
               "remaining_ms":%d,"source_color":-65536,"source_label":"灰烬蛛","dispel":1}
            ]}
            """.formatted(StatusEffectStore.INDEFINITE_REMAINING_MS);
        var result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertFalse(result.isParseError(), result.logMessage());
        var wound = StatusEffectStore.snapshot().get(0);
        String tooltip = BuffBarPanel.tooltipText(wound);
        assertTrue(tooltip.contains("来源: 灰烬蛛"), "不能将实际攻击者改写成丹药");
        assertTrue(tooltip.contains("持续"));
        assertFalse(tooltip.contains("剩余:"), "无到期时间不能显示巨大的分钟数");
        assertTrue(StatusEffectIcons.textureFor(wound.id()).endsWith("/bleeding.png"),
            "持续伤害按具体效果选择图标，流血不能标成毒");

        long now = System.currentTimeMillis();
        StatusEffectStore.presentation(now, 8);
        var later = StatusEffectStore.presentation(now + 60_000, 8);
        assertEquals(StatusEffectStore.INDEFINITE_REMAINING_MS, later.items().get(0).remainingMs());
        StatusEffectStore.replace(List.of(), now + 60_000);
        assertTrue(StatusEffectStore.presentation(now + 61_000, 8).items().isEmpty(),
            "持续状态仍须在服务端清除后退出");
    }

    private static StatusEffectStore.Effect effect(
        String id, String name, StatusEffectStore.Kind kind, int stacks, long remainingMs
    ) {
        return new StatusEffectStore.Effect(id, name, kind, stacks, remainingMs, 0xFFAA5533, "source", 2);
    }

    // ─── 布局收起/展开（无 buff 不占位；有 buff 按 slot 数横向展开） ───────────

    @Test
    void requiredWidthAndHeightCollapseToZeroWhenEmpty() {
        assertEquals(0, BuffBarPanel.requiredWidth(0), "无 buff 时宽度须收起为 0，不留占位空隙");
        assertEquals(0, BuffBarPanel.requiredHeight(0), "无 buff 时高度须收起为 0，HUD 沉浸原则不常驻空壳");
    }

    @Test
    void requiredWidthAndHeightForSingleEffect() {
        assertEquals(BuffBarPanel.SLOT_SIZE, BuffBarPanel.requiredWidth(1), "单个 buff 无需 gap，宽度=一个槽位");
        assertEquals(BuffBarPanel.SLOT_SIZE, BuffBarPanel.requiredHeight(1));
    }

    @Test
    void requiredWidthAccountsForGapsBetweenSlots() {
        int expected = 3 * BuffBarPanel.SLOT_SIZE + 2 * BuffBarPanel.SLOT_GAP;
        assertEquals(expected, BuffBarPanel.requiredWidth(3),
            "N 个槽位之间只有 N-1 个 gap，不是 N 个（否则末尾多出一截空白）");
        assertEquals(BuffBarPanel.SLOT_SIZE, BuffBarPanel.requiredHeight(3), "高度恒为单槽高，不随数量增长");
    }

    @Test
    void requiredWidthNeverNegativeForDefensiveNegativeCount() {
        assertEquals(0, BuffBarPanel.requiredWidth(-1), "越界防御：负数视同空列表");
        assertEquals(0, BuffBarPanel.requiredHeight(-1));
    }

    // ─── 排序（沿用 HUD 顶栏优先级：DoT > Control > Debuff > Buff > Unknown，但不截断 Top-8） ───

    @Test
    void sortedEffectsOrdersByKindRankThenRemaining() {
        StatusEffectStore.Effect buff = effect("buff_a", "增益", StatusEffectStore.Kind.BUFF, 1, 5_000);
        StatusEffectStore.Effect dot = effect("dot_a", "灼烧", StatusEffectStore.Kind.DOT, 1, 9_000);
        StatusEffectStore.Effect control = effect("ctrl_a", "眩晕", StatusEffectStore.Kind.CONTROL, 1, 1_000);

        List<StatusEffectStore.Effect> sorted = BuffBarPanel.sortedEffects(List.of(buff, dot, control));

        assertEquals(List.of(dot, control, buff), sorted,
            "DoT 排最前、Control 次之、Buff 最后，与 HUD 顶栏视觉语言一致");
    }

    @Test
    void sortedEffectsKeepsAllEntriesUnlikeHudTopBar() {
        // HUD 顶栏截断 Top-8，背包界面空间富余应展示全部——这是本面板与 HUD 侧的关键差异点。
        List<StatusEffectStore.Effect> nine = new java.util.ArrayList<>();
        for (int i = 0; i < 9; i++) {
            nine.add(effect("e" + i, "e" + i, StatusEffectStore.Kind.BUFF, 1, i * 1000L));
        }
        List<StatusEffectStore.Effect> sorted = BuffBarPanel.sortedEffects(nine);
        assertEquals(9, sorted.size(), "背包 buff 条不做 Top-8 截断，9 个全部保留");
    }

    @Test
    void sortedEffectsWithinSameKindOrdersByRemainingAscending() {
        StatusEffectStore.Effect long1 = effect("b1", "b1", StatusEffectStore.Kind.BUFF, 1, 20_000);
        StatusEffectStore.Effect short1 = effect("b2", "b2", StatusEffectStore.Kind.BUFF, 1, 3_000);

        List<StatusEffectStore.Effect> sorted = BuffBarPanel.sortedEffects(List.of(long1, short1));
        assertEquals(List.of(short1, long1), sorted, "同 kind 内按剩余时间升序（快消失的更靠前，提醒紧迫感）");
    }

    @Test
    void sortedEffectsOnEmptyListReturnsEmpty() {
        assertTrue(BuffBarPanel.sortedEffects(List.of()).isEmpty());
    }

    // ─── 命中测试（悬浮检测，供 tooltip 定位用） ─────────────────────────────

    @Test
    void effectAtScreenReturnsNullWhenListEmpty() {
        assertNull(BuffBarPanel.effectAtScreen(List.of(), 10, 10, 15, 15));
    }

    @Test
    void effectAtScreenReturnsNullWhenMouseAboveOrBelowRow() {
        StatusEffectStore.Effect e = effect("a", "A", StatusEffectStore.Kind.BUFF, 1, 5_000);
        List<StatusEffectStore.Effect> list = List.of(e);
        assertNull(BuffBarPanel.effectAtScreen(list, 10, 10, 15, 9),
            "鼠标 y 在槽位上边界之上 → 不命中");
        assertNull(BuffBarPanel.effectAtScreen(list, 10, 10, 15, 10 + BuffBarPanel.SLOT_SIZE),
            "鼠标 y 恰好等于下边界（半开区间不含） → 不命中");
    }

    @Test
    void effectAtScreenHitsFirstSlotAtLeftEdge() {
        StatusEffectStore.Effect e = effect("a", "A", StatusEffectStore.Kind.BUFF, 1, 5_000);
        StatusEffectStore.Effect result = BuffBarPanel.effectAtScreen(List.of(e), 100, 50, 100, 55);
        assertSame(e, result, "命中第一个槽的左上角边界（含）应返回该 effect");
    }

    @Test
    void effectAtScreenHitsSecondSlotNotFirst() {
        StatusEffectStore.Effect first = effect("a", "A", StatusEffectStore.Kind.BUFF, 1, 5_000);
        StatusEffectStore.Effect second = effect("b", "B", StatusEffectStore.Kind.BUFF, 1, 5_000);
        int secondSlotX = 100 + BuffBarPanel.SLOT_SIZE + BuffBarPanel.SLOT_GAP;

        StatusEffectStore.Effect result = BuffBarPanel.effectAtScreen(
            List.of(first, second), 100, 50, secondSlotX + 2, 55
        );
        assertSame(second, result, "第二个槽命中须返回第二个 effect，不是第一个");
    }

    @Test
    void effectAtScreenReturnsNullWhenMouseInGapBetweenSlots() {
        StatusEffectStore.Effect first = effect("a", "A", StatusEffectStore.Kind.BUFF, 1, 5_000);
        StatusEffectStore.Effect second = effect("b", "B", StatusEffectStore.Kind.BUFF, 1, 5_000);
        int gapX = 100 + BuffBarPanel.SLOT_SIZE + 1; // gap 区域内（SLOT_GAP=3 时中点）

        StatusEffectStore.Effect result = BuffBarPanel.effectAtScreen(
            List.of(first, second), 100, 50, gapX, 55
        );
        assertNull(result, "槽与槽之间的 gap 区域不应命中任何 buff（不能误触发相邻 buff 的 tooltip）");
    }

    @Test
    void effectAtScreenReturnsNullWhenMouseFarRightOfAllSlots() {
        StatusEffectStore.Effect e = effect("a", "A", StatusEffectStore.Kind.BUFF, 1, 5_000);
        assertNull(BuffBarPanel.effectAtScreen(List.of(e), 100, 50, 1000, 55));
    }

    // ─── 剩余时间比例条归一化 ──────────────────────────────────────────────

    @Test
    void remainingNormClampsToZeroWhenExpiredOrNegative() {
        assertEquals(0f, BuffBarPanel.remainingNorm(0L));
        assertEquals(0f, BuffBarPanel.remainingNorm(-500L), "防御性：负 remainingMs 不应产生负比例");
    }

    @Test
    void remainingNormClampsToOneWhenAboveCap() {
        assertEquals(1f, BuffBarPanel.remainingNorm(120_000L), "远超 30s 归一化上限的仍 clamp 到满条，不越界");
    }

    @Test
    void remainingNormAtHalfCapIsHalf() {
        assertEquals(0.5f, BuffBarPanel.remainingNorm(15_000L), 0.001f);
    }

    // ─── kind → 负面色判定（决定剩余时间条走白色还是红色倒计时） ────────────────

    @Test
    void negativeKindCoversDotControlDebuff() {
        assertTrue(BuffBarPanel.isNegativeKind(StatusEffectStore.Kind.DOT));
        assertTrue(BuffBarPanel.isNegativeKind(StatusEffectStore.Kind.CONTROL));
        assertTrue(BuffBarPanel.isNegativeKind(StatusEffectStore.Kind.DEBUFF));
    }

    @Test
    void negativeKindExcludesBuffAndUnknown() {
        assertFalse(BuffBarPanel.isNegativeKind(StatusEffectStore.Kind.BUFF));
        assertFalse(BuffBarPanel.isNegativeKind(StatusEffectStore.Kind.UNKNOWN));
    }

    // ─── 剩余时间格式化：<60s 一位小数秒；>=60s 分+秒 ─────────────────────

    @Test
    void formatRemainingUnderOneMinuteShowsOneDecimalSeconds() {
        assertEquals("0.0s", StatusPanelExtension.formatRemaining(0L));
        assertEquals("4.5s", StatusPanelExtension.formatRemaining(4_500L));
        assertEquals("0.1s", StatusPanelExtension.formatRemaining(150L));
    }

    @Test
    void formatRemainingNegativeClampsToZero() {
        assertEquals("0.0s", StatusPanelExtension.formatRemaining(-1_000L), "防御性：负值不应打出负数秒");
    }

    @Test
    void formatRemainingJustUnderOneMinuteDoesNotRoundUpToSixtySeconds() {
        // 59999ms 若用四舍五入的 %.1f 直接格式化会显示 "60.0s"（和下一档格式撞脸）；
        // 用 floor 取十分位后应稳定落在 <60s 档内。
        assertEquals("59.9s", StatusPanelExtension.formatRemaining(59_999L));
    }

    @Test
    void formatRemainingAtOneMinuteBoundarySwitchesToMinuteSecondFormat() {
        assertEquals("1m 0s", StatusPanelExtension.formatRemaining(60_000L), "恰好 60000ms 属于 >=60s 档");
    }

    @Test
    void formatRemainingAboveOneMinuteShowsMinutesAndSeconds() {
        assertEquals("1m 5s", StatusPanelExtension.formatRemaining(65_000L));
        assertEquals("2m 30s", StatusPanelExtension.formatRemaining(150_000L));
    }

    @Test
    void formatRemainingManyMinutesDoesNotOverflowToHours() {
        // 无更高档位（小时）——规格只要求分+秒，长时长仍以分钟数线性增长表示。
        assertEquals("10m 0s", StatusPanelExtension.formatRemaining(600_000L));
    }

    // ─── tooltip 与其它状态详情使用同一份语义 ───

    @Test
    void tooltipTextIncludesNameStacksSourceAndDispelDifficulty() {
        StatusEffectStore.Effect e = effect("burn", "灼烧", StatusEffectStore.Kind.DOT, 3, 4_500L);
        String text = BuffBarPanel.tooltipText(e);
        assertTrue(text.contains("灼烧"), "须包含 buff 名字");
        assertTrue(text.contains("×3"), "须包含层数");
        assertTrue(text.contains("source"), "须包含来源标签（复用 tooltipFor 的来源行）");
        assertTrue(text.contains("2/5"), "须包含驱散难度（复用 tooltipFor 的驱散难度行）");
    }

    @Test
    void tooltipTextUsesMinuteSecondFormat() {
        StatusEffectStore.Effect e = effect("slow", "迟缓", StatusEffectStore.Kind.DEBUFF, 1, 90_000L);
        String text = BuffBarPanel.tooltipText(e);
        assertTrue(text.contains("剩余: 1m 30s"),
            () -> "长效 buff 的 tooltip 剩余行须显示分+秒格式，实际=" + text);
        assertFalse(text.contains("90.0s"), "长时长应显示分秒");
    }

    @Test
    void tooltipTextForShortDurationKeepsSecondsFormat() {
        StatusEffectStore.Effect e = effect("burn", "灼烧", StatusEffectStore.Kind.DOT, 1, 4_500L);
        String text = BuffBarPanel.tooltipText(e);
        assertTrue(text.contains("剩余: 4.5s"), () -> "短效 buff 应保留一位小数秒格式，实际=" + text);
    }

    @Test
    void tooltipTextForSingleStackEffectOmitsMultiplierSuffix() {
        // StatusPanelExtension.tooltipFor 只在 stacks>=2 时才加 "×N" 后缀。
        StatusEffectStore.Effect e = effect("solo", "孤立效果", StatusEffectStore.Kind.BUFF, 1, 10_000L);
        String text = BuffBarPanel.tooltipText(e);
        assertTrue(text.contains("孤立效果"));
        assertFalse(text.contains("×1"), "单层 buff 不应打出 ×1 冗余后缀");
    }

    @Test
    void tooltipTextTracksLiveEffectNotStaleSnapshot() {
        // 覆盖"tooltip 绑定须跟随刷新，不指向过期 buff"——同一 id 的 effect 剩余时间变化后，
        // 用新 Effect 记录重新调用 tooltipText 应立刻反映新值，而不是复用旧字符串。
        StatusEffectStore.Effect before = effect("dot_x", "灼烧", StatusEffectStore.Kind.DOT, 1, 10_000L);
        StatusEffectStore.Effect after = effect("dot_x", "灼烧", StatusEffectStore.Kind.DOT, 1, 2_000L);

        String textBefore = BuffBarPanel.tooltipText(before);
        String textAfter = BuffBarPanel.tooltipText(after);

        assertTrue(textBefore.contains("剩余: 10.0s"));
        assertTrue(textAfter.contains("剩余: 2.0s"),
            () -> "effect 剩余时间刷新后 tooltip 须反映最新值，实际=" + textAfter);
    }
}
