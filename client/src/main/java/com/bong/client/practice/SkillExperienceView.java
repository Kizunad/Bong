package com.bong.client.practice;

import java.util.Locale;

/** 技艺经验和现有效果的展示投影，与 Inspect 布局解耦。 */
public final class SkillExperienceView {
    private SkillExperienceView() {}
    public static java.util.List<com.bong.client.skill.SkillMilestoneSnapshot> recentMilestonesForSkill(
        com.bong.client.skill.SkillId skill
    ) {
        java.util.List<com.bong.client.skill.SkillMilestoneSnapshot> all =
            com.bong.client.skill.SkillMilestoneStore.snapshot();
        java.util.ArrayList<com.bong.client.skill.SkillMilestoneSnapshot> filtered = new java.util.ArrayList<>();
        for (int i = all.size() - 1; i >= 0 && filtered.size() < 3; i--) {
            com.bong.client.skill.SkillMilestoneSnapshot snapshot = all.get(i);
            if (snapshot != null && snapshot.skill() == skill) {
                filtered.add(snapshot);
            }
        }
        return java.util.List.copyOf(filtered);
    }

    public static java.util.List<com.bong.client.skill.SkillRecentEventStore.Entry> recentEventsForSkill(
        com.bong.client.skill.SkillId skill
    ) {
        java.util.List<com.bong.client.skill.SkillRecentEventStore.Entry> all =
            com.bong.client.skill.SkillRecentEventStore.snapshot();
        java.util.ArrayList<com.bong.client.skill.SkillRecentEventStore.Entry> filtered = new java.util.ArrayList<>();
        for (com.bong.client.skill.SkillRecentEventStore.Entry entry : all) {
            if (entry != null && entry.skill() == skill) {
                filtered.add(entry);
                if (filtered.size() >= 3) break;
            }
        }
        return java.util.List.copyOf(filtered);
    }

    public static String formatSkillRecentEventLine(com.bong.client.skill.SkillRecentEventStore.Entry entry) {
        if (entry == null) return "（暂无）";
        return switch (entry.kind()) {
            case "xp_gain" -> entry.text();
            case "lv_up" -> entry.text();
            case "cap_changed" -> entry.text();
            case "scroll_used" -> entry.text();
            default -> entry.text();
        };
    }

    public static String formatSkillMilestoneLine(com.bong.client.skill.SkillMilestoneSnapshot milestone) {
        if (milestone == null) return "（暂无）";
        String narration = milestone.narration();
        if (narration != null && !narration.isBlank()) {
            return "Lv." + milestone.newLv() + " · " + narration;
        }
        return "Lv." + milestone.newLv() + " · t" + milestone.achievedAt() + " · 累计 " + milestone.totalXpAt() + " XP";
    }

    public static String formatSkillLevelLine(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "Lv.0 / effective 0 / cap 10";
        String line = "Lv." + entry.lv() + " / effective " + entry.effectiveLv() + " / cap " + entry.cap();
        if (entry.lv() > entry.cap()) {
            line += " · 境界压制";
        }
        return line;
    }

    public static String formatSkillProgressLine(com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (entry == null) return "当前 XP 0 / 100 · 累计 0";
        if (entry.lv() >= 10) {
            return "Lv.10 已满 · 累计 " + entry.totalXp() + " XP";
        }
        return "当前 XP " + entry.xp() + " / " + entry.xpToNext() + " · 累计 " + entry.totalXp();
    }

    public static String formatSkillCurrentEffect(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "当前效果：尚未入门。";
        int lv = entry.effectiveLv();
        return "当前效果：" + switch (skill) {
            case HERBALISM -> herbalismCurrentEffect(lv);
            case ALCHEMY -> alchemyCurrentEffect(lv);
            case FORGING -> forgingCurrentEffect(lv);
            case COMBAT -> combatCurrentEffect(lv);
            case MINERAL -> mineralCurrentEffect(lv);
            case CULTIVATION -> cultivationCurrentEffect(lv);
        };
    }

    public static String formatSkillNextEffect(com.bong.client.skill.SkillId skill, com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (skill == null || entry == null) return "下一阶：继续修习可见首层变化。";
        int effective = entry.effectiveLv();
        if (entry.lv() >= 10) {
            return "下一阶：已至极限，后续只看境界能否完全承住这门手艺。";
        }
        if (entry.lv() > entry.cap()) {
            return "下一阶：真实等级已高于境界上限；待突破后，压住的效果会直接放开。";
        }
        int nextLv = Math.min(10, effective + 1);
        return "下一阶：effective 提到 " + nextLv + " 时，"
            + switch (skill) {
                case HERBALISM -> herbalismNextEffect(nextLv);
                case ALCHEMY -> alchemyNextEffect(nextLv);
                case FORGING -> forgingNextEffect(nextLv);
                case COMBAT -> combatNextEffect(nextLv);
                case MINERAL -> mineralNextEffect(nextLv);
                case CULTIVATION -> cultivationNextEffect(nextLv);
            };
    }

    public static String formatSkillHint(com.bong.client.skill.SkillSetSnapshot.Entry entry) {
        if (entry == null) return "境界不足时，高于上限的等级暂不生效。";
        if (entry.lv() > entry.cap()) {
            return "你已练到更高层次，当前按境界允许的等级发挥。";
        }
        if (entry.cap() < 10) {
            return "当前境界最多承到等级 " + entry.cap() + "；继续突破后，高等级效果会自然放开。";
        }
        return "当前境界不再压制这门技艺。";
    }

    private static String herbalismCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "手动采集 %.1fs，加成种子掉率 +%s%%，品质偏移 +%s%%。%s",
            herbalismManualDurationDelta(effectiveLv),
            formatPercent1(herbalismSeedBonus(effectiveLv)),
            formatInt(herbalismQualityBias(effectiveLv)),
            herbalismAutoText(effectiveLv)
        );
    }

    private static String herbalismNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "手动采集 %.1fs，种子掉率 +%s%%，品质偏移 +%s%%。%s",
            herbalismManualDurationDelta(nextLv),
            formatPercent1(herbalismSeedBonus(nextLv)),
            formatInt(herbalismQualityBias(nextLv)),
            herbalismAutoText(nextLv)
        );
    }

    private static String alchemyCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "火候容差 ×%s，坏副作用权重 ×%s，丹毒排异 +%s%%。",
            formatPercent2(alchemyToleranceScale(effectiveLv)),
            formatPercent2(alchemyBadWeightScale(effectiveLv)),
            formatPercent1(alchemyPurgeBonus(effectiveLv) * 100.0)
        );
    }

    private static String alchemyNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "火候容差 ×%s，坏副作用权重 ×%s，丹毒排异 +%s%%。",
            formatPercent2(alchemyToleranceScale(nextLv)),
            formatPercent2(alchemyBadWeightScale(nextLv)),
            formatPercent1(alchemyPurgeBonus(nextLv) * 100.0)
        );
    }

    private static String forgingCurrentEffect(int effectiveLv) {
        return String.format(
            Locale.ROOT,
            "淬火命中窗 +%s tick，允许失误 +%s，铭文失败率 -%s%%。",
            formatInt(forgingWindowBonus(effectiveLv)),
            formatInt(forgingAllowedMiss(effectiveLv)),
            formatPercent1(forgingFailureReduction(effectiveLv) * 100.0)
        );
    }

    private static String forgingNextEffect(int nextLv) {
        return String.format(
            Locale.ROOT,
            "淬火命中窗 +%s tick，允许失误 +%s，铭文失败率 -%s%%。",
            formatInt(forgingWindowBonus(nextLv)),
            formatInt(forgingAllowedMiss(nextLv)),
            formatPercent1(forgingFailureReduction(nextLv) * 100.0)
        );
    }

    private static String combatCurrentEffect(int effectiveLv) {
        return "击杀、实战与截脉对练会增长此项；当前只作熟练度记录，随实战积累磨练。"
            + " 实效 Lv." + effectiveLv + "。";
    }

    private static String combatNextEffect(int nextLv) {
        return "实战熟练度将提到 Lv." + nextLv + "。";
    }

    private static String mineralCurrentEffect(int effectiveLv) {
        return "采矿、辨矿与落袋会增长此项；当前只作熟练度记录。实效 Lv." + effectiveLv + "。";
    }

    private static String mineralNextEffect(int nextLv) {
        return "采矿熟练度将提到 Lv." + nextLv + "。";
    }

    private static String cultivationCurrentEffect(int effectiveLv) {
        return "开脉与突破会增长此项；境界仍是根本，本项只记行功熟练。实效 Lv." + effectiveLv + "。";
    }

    private static String cultivationNextEffect(int nextLv) {
        return "行功熟练度将提到 Lv." + nextLv + "；不替代境界，只辅助展示修行履历。";
    }

    private static String herbalismAutoText(int effectiveLv) {
        if (effectiveLv < 3) return "自动采集未开。";
        return String.format(Locale.ROOT, "自动采集已开，时长 %.1fs。", herbalismAutoDuration(effectiveLv));
    }

    private static double herbalismManualDurationDelta(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, -0.2}, {3, -0.5}, {5, -1.0}, {7, -1.2}, {10, -1.5}
        });
    }

    private static double herbalismSeedBonus(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 2.0}, {3, 5.0}, {5, 10.0}, {7, 15.0}, {10, 25.0}
        });
    }

    private static double herbalismQualityBias(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 5.0}, {3, 10.0}, {5, 15.0}, {7, 20.0}, {10, 30.0}
        });
    }

    private static double herbalismAutoDuration(int effectiveLv) {
        if (effectiveLv < 3) return 0.0;
        return interpolate(effectiveLv, new double[][] {
            {3, 8.0}, {5, 6.0}, {7, 5.0}, {10, 5.0}
        });
    }

    private static double alchemyToleranceScale(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 1.00}, {1, 1.05}, {3, 1.15}, {5, 1.25}, {7, 1.35}, {10, 1.50}
        });
    }

    private static double alchemyBadWeightScale(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 1.00}, {1, 0.95}, {3, 0.85}, {5, 0.75}, {7, 0.60}, {10, 0.40}
        });
    }

    private static double alchemyPurgeBonus(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.00}, {1, 0.02}, {3, 0.05}, {5, 0.10}, {7, 0.15}, {10, 0.25}
        });
    }

    private static double forgingWindowBonus(int effectiveLv) {
        return Math.round(interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 1.0}, {3, 3.0}, {5, 5.0}, {7, 6.0}, {10, 8.0}
        }));
    }

    private static double forgingAllowedMiss(int effectiveLv) {
        return Math.round(interpolate(effectiveLv, new double[][] {
            {0, 0.0}, {1, 0.0}, {3, 1.0}, {5, 1.0}, {7, 2.0}, {10, 3.0}
        }));
    }

    private static double forgingFailureReduction(int effectiveLv) {
        return interpolate(effectiveLv, new double[][] {
            {0, 0.00}, {1, 0.03}, {3, 0.10}, {5, 0.15}, {7, 0.22}, {10, 0.30}
        });
    }

    private static double interpolate(int lv, double[][] points) {
        if (points == null || points.length == 0) return 0.0;
        if (lv <= points[0][0]) return points[0][1];
        for (int i = 0; i < points.length - 1; i++) {
            double l0 = points[i][0];
            double v0 = points[i][1];
            double l1 = points[i + 1][0];
            double v1 = points[i + 1][1];
            if (lv <= l1) {
                double t = (lv - l0) / (l1 - l0);
                return v0 + (v1 - v0) * t;
            }
        }
        return points[points.length - 1][1];
    }

    private static String formatPercent1(double value) {
        return String.format(Locale.ROOT, "%.1f", value);
    }

    private static String formatPercent2(double value) {
        return String.format(Locale.ROOT, "%.2f", value);
    }

    private static String formatInt(double value) {
        return Integer.toString((int) Math.round(value));
    }

}
