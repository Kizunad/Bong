package com.bong.client.alchemy.state;

import java.util.ArrayList;
import java.util.List;

// plan-alchemy-v1 P6 — 丹方残卷本地 Store（含翻页/学习助手）。
public final class RecipeScrollStore {
    public record RecipeEntry(
        String id,
        String displayName,
        String bodyText,
        String author,
        String era,
        int maxKnown
    ) {
        public RecipeEntry(String id, String displayName, String bodyText) {
            this(id, displayName, bodyText, "散修 刘三", "末法 十二年", 8);
        }
    }

    public record Snapshot(List<RecipeEntry> learned, int currentIndex) {
        public static Snapshot defaults() {
            String kaimaiBody = String.join("\n",
                "§e凡炼开脉，先取§6【开脉草】§e三株，",
                "§e须是灵气 > 0.3 处所生，枯者不可用。",
                "§e佐以§6【灵水】§e一勺，以凝脉引。",
                "§e入炉前须净手，炉温起于温火，",
                "§e渐旺至 §6六成火§e（约 0.60），",
                "§e持 §6二百息§e（约 200 ticks），",
                "§e期间徐徐注真元，总计 §b一十五§e。",
                "",
                "§6火候偏差：",
                "§7 · 温差 ±一成（0.10）内为佳",
                "§7 · 时差 ±三十息（30）尚可",
                "§7 · 逾则丹废、炉裂、人伤",
                "",
                "§6成则：",
                "§f 开脉丹 · 推一经三成",
                "§7 副毒：醇（Mellow）半钱",
                "",
                "§c\u26A0 切记",
                "§7 · 体内醇毒未化尽，切勿再服",
                "§7 · 经脉愈固，化毒愈速"
            );
            List<RecipeEntry> seed = List.of(
                new RecipeEntry("kaimai_pill", "开脉丹方", kaimaiBody, "散修 刘三", "末法 十二年", 8),
                new RecipeEntry("huiyuan_pill", "回元丹方",
                    "§e温补真元，低阶筑基常备。\n§7配料 / 火候 / 偏差 / 残缺规则将在 Slice B 接入 Store。",
                    "散修 刘三", "末法 十二年", 8),
                new RecipeEntry("duming_san", "赌命散方",
                    "§c烈性偏方，反噬极大。\n§7老炼丹师嗤之以鼻。",
                    "散修 刘三", "末法 十二年", 8)
            );
            return new Snapshot(seed, 0);
        }

        public static Snapshot empty() {
            return new Snapshot(List.of(), 0);
        }

        public RecipeEntry current() {
            if (learned.isEmpty()) return null;
            int i = Math.floorMod(currentIndex, learned.size());
            return learned.get(i);
        }
    }

    private static volatile Snapshot snapshot = Snapshot.defaults();

    private RecipeScrollStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.defaults() : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.defaults();
    }

    public static void turn(int delta) {
        Snapshot s = snapshot;
        if (s.learned.isEmpty()) return;
        int ni = Math.floorMod(s.currentIndex + delta, s.learned.size());
        snapshot = new Snapshot(s.learned, ni);
    }

    public static boolean learn(RecipeEntry e) {
        if (e == null) return false;
        Snapshot s = snapshot;
        for (RecipeEntry r : s.learned) {
            if (r.id().equals(e.id())) return false;
        }
        List<RecipeEntry> next = new ArrayList<>(s.learned);
        next.add(e);
        snapshot = new Snapshot(List.copyOf(next), s.currentIndex);
        return true;
    }
}
