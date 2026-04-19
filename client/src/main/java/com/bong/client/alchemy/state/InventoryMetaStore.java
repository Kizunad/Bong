package com.bong.client.alchemy.state;

import java.util.List;

// plan-alchemy-v1 P6 — 背包 tab / 重量 / 键位提示快照本地 Store。
public final class InventoryMetaStore {
    public record TabEntry(String label, boolean active) {}

    public record Snapshot(
        List<TabEntry> tabs,
        int activeTabIndex,
        float weightCurrent,
        float weightMax,
        List<String> keybindHints
    ) {
        public static Snapshot empty() {
            return new Snapshot(
                List.of(
                    new TabEntry("主背包 5×7", true),
                    new TabEntry("小口袋 3×3", false),
                    new TabEntry("前挂包 3×4", false)
                ),
                0,
                12.3f,
                50.0f,
                List.of(
                    "§8键位：Shift+左键 快速投入炉槽（自动选空槽）",
                    "§8右键炉槽 取回材料（起炉前）",
                    "§c\u26A0 起炉后投料槽锁定 · 材料不可退"
                )
            );
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private InventoryMetaStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
