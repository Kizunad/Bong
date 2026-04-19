package com.bong.client.botany;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;

/**
 * plan-skill-v1 §9 P2：Botany 单项视图迁为 {@link SkillSetStore} 派生视图，
 * 不再独立持有数据。保留公开 API（{@link #snapshot()} / {@link #replace} / {@link #clearOnDisconnect}）
 * 兼容现有消费方（HUD bootstrap、viewmodel 等），等 P7 一起移除。
 *
 * <p>{@link #replace(BotanySkillViewModel)} 把 VM 镜像回 {@link SkillSetStore} 的
 * {@link SkillId#HERBALISM} 条目，保持老 handler 的 setter 语义。
 */
public final class BotanySkillStore {
    private BotanySkillStore() {}

    /** 派生视图：从 SkillSetStore 读出 herbalism 条目映射回 legacy VM。 */
    public static BotanySkillViewModel snapshot() {
        SkillSetSnapshot.Entry e = SkillSetStore.snapshot().get(SkillId.HERBALISM);
        if (e == null) return BotanySkillViewModel.defaultView();
        // plan §2.1 门槛 Lv.3 解锁自动采集；BotanySkillViewModel 里 autoUnlockLevel 硬编码 3。
        return BotanySkillViewModel.create(e.lv(), e.xp(), e.xpToNext(), 3);
    }

    /**
     * 兼容入口：把 legacy VM 镜像回 {@link SkillSetStore}，让 InspectScreen 等新消费方也能看到。
     *
     * <p>P7 会直接删除本 store；届时所有 server 侧老 payload 都迁到新 skill 通道，本入口失效。
     */
    public static void replace(BotanySkillViewModel next) {
        if (next == null) return;
        SkillSetSnapshot.Entry cur = SkillSetStore.snapshot().get(SkillId.HERBALISM);
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(
            next.level(),
            next.xp(),
            next.xpToNextLevel(),
            Math.max(cur.totalXp(), next.xp()),
            cur.cap(),
            cur.recentGainXp(),
            cur.recentGainMillis()
        );
        SkillSetStore.updateEntry(SkillId.HERBALISM, entry);
    }

    public static void clearOnDisconnect() {
        SkillSetStore.clearOnDisconnect();
    }

    public static void resetForTests() {
        SkillSetStore.resetForTests();
    }
}
