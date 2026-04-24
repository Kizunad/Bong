package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillRecentEventStore;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.regex.Pattern;

/**
 * plan-skill-v1 §8 四种 skill channel 事件客户端处理：
 * <ul>
 *   <li>{@code skill_xp_gain} — 单次 XP 进账，累加到对应 skill 的 recent_gain 并更新 xp/totalXp</li>
 *   <li>{@code skill_lv_up} — 跨级，刷新 lv + xpToNext</li>
 *   <li>{@code skill_cap_changed} — plan §4 境界软挂钩</li>
 *   <li>{@code skill_scroll_used} — plan §3.2，同时弹 tooltip 反馈（P2 阶段仅更新 store）</li>
 * </ul>
 *
 * <p>这 4 个类型对应 Redis channel {@code bong:skill/*}（server→agent）；server 端
 * 何时同步向 client 推送此 4 类 CustomPayload 由各 plan 触发点接入决定（P3+）。本 handler
 * 先注册好分发路径，等 server 端触发上线即可零改动吃数据。
 *
 * <p>Server 侧同时保留老的 {@code botany_skill} 通道（P2 不移除，兼容）—— 该 handler 同时
 * 把 herbalism 镜像到本 store（见 {@link BotanySkillHandler}）。
 */
public final class SkillEventHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    /** plan §2.1 xp_to_next(lv) = 100 * (lv+1)^2；client 派生曲线与 server 对齐。 */
    private static long xpToNext(int lv) {
        if (lv >= 10) return Long.MAX_VALUE;
        long n = (long) lv + 1L;
        return 100L * n * n;
    }

    private SkillEventHandler() {}

    public static ServerDataHandler xpGainHandler() {
        return envelope -> handleXpGain(envelope);
    }

    public static ServerDataHandler lvUpHandler() {
        return envelope -> handleLvUp(envelope);
    }

    public static ServerDataHandler capChangedHandler() {
        return envelope -> handleCapChanged(envelope);
    }

    public static ServerDataHandler scrollUsedHandler() {
        return envelope -> handleScrollUsed(envelope);
    }

    private static ServerDataDispatch handleXpGain(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        SkillId skill = SkillId.fromWire(readString(p, "skill"));
        Long amount = readLong(p, "amount");
        if (skill == null || amount == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skill_xp_gain: invalid or missing skill/amount");
        }
        SkillSetSnapshot snap = SkillSetStore.snapshot();
        SkillSetSnapshot.Entry cur = snap.get(skill);
        long newXp = Math.max(0L, cur.xp() + amount);
        long newTotal = cur.totalXp() + amount;
        // 不跨级 —— lv_up 事件单独处理；这里只加 xp。若无 lv_up 而 xp 超过 xpToNext，暂保留数值，
        // 等 lv_up 事件到达修正；避免 race 把 lv 算错。
        SkillSetSnapshot.Entry next = new SkillSetSnapshot.Entry(
            cur.lv(), newXp, cur.xpToNext(), newTotal, cur.cap(),
            amount, System.currentTimeMillis()
        );
        SkillSetStore.updateEntry(skill, next);
        SkillRecentEventStore.append(new SkillRecentEventStore.Entry(
            skill,
            "xp_gain",
            "+" + amount + " XP",
            System.currentTimeMillis()
        ));
        return ServerDataDispatch.handled(envelope.type(),
            "skill_xp_gain +" + amount + " applied to " + skill.wireId());
    }

    private static ServerDataDispatch handleLvUp(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        SkillId skill = SkillId.fromWire(readString(p, "skill"));
        Integer newLv = readInt(p, "new_lv");
        if (skill == null || newLv == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skill_lv_up: invalid or missing skill/new_lv");
        }
        SkillSetSnapshot.Entry cur = SkillSetStore.snapshot().get(skill);
        SkillSetSnapshot.Entry next = new SkillSetSnapshot.Entry(
            newLv, 0L, xpToNext(newLv), cur.totalXp(), cur.cap(),
            cur.recentGainXp(), cur.recentGainMillis()
        );
        SkillSetStore.updateEntry(skill, next);
        SkillRecentEventStore.append(new SkillRecentEventStore.Entry(
            skill,
            "lv_up",
            "升至 Lv." + newLv,
            System.currentTimeMillis()
        ));
        return ServerDataDispatch.handled(envelope.type(),
            "skill_lv_up " + skill.wireId() + " → Lv." + newLv);
    }

    private static ServerDataDispatch handleCapChanged(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        SkillId skill = SkillId.fromWire(readString(p, "skill"));
        Integer newCap = readInt(p, "new_cap");
        if (skill == null || newCap == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skill_cap_changed: invalid or missing skill/new_cap");
        }
        SkillSetSnapshot.Entry cur = SkillSetStore.snapshot().get(skill);
        SkillSetSnapshot.Entry next = new SkillSetSnapshot.Entry(
            cur.lv(), cur.xp(), cur.xpToNext(), cur.totalXp(), newCap,
            cur.recentGainXp(), cur.recentGainMillis()
        );
        SkillSetStore.updateEntry(skill, next);
        SkillRecentEventStore.append(new SkillRecentEventStore.Entry(
            skill,
            "cap_changed",
            "cap 调整为 " + newCap,
            System.currentTimeMillis()
        ));
        return ServerDataDispatch.handled(envelope.type(),
            "skill_cap_changed " + skill.wireId() + " cap=" + newCap);
    }

    private static ServerDataDispatch handleScrollUsed(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        SkillId skill = SkillId.fromWire(readString(p, "skill"));
        String scrollId = readString(p, "scroll_id");
        Long granted = readLong(p, "xp_granted");
        Boolean dup = readBool(p, "was_duplicate");
        if (skill == null || scrollId == null || granted == null || dup == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skill_scroll_used: invalid or missing fields");
        }
        SkillSetStore.replace(SkillSetStore.snapshot().withConsumedScrolls(
            mergeConsumedScroll(scrollId)
        ));
        // P2 阶段仅更新 store 用作 UI 最近动作提示；不消耗残卷视为 0 xp，不改 entry xp 字段。
        if (dup) {
            return ServerDataDispatch.handled(envelope.type(),
                "skill_scroll_used " + skill.wireId() + " (此卷已悟，不计 xp)");
        }
        SkillSetSnapshot.Entry cur = SkillSetStore.snapshot().get(skill);
        SkillSetSnapshot.Entry next = new SkillSetSnapshot.Entry(
            cur.lv(), cur.xp(), cur.xpToNext(), cur.totalXp(), cur.cap(),
            granted, System.currentTimeMillis()
        );
        SkillSetStore.updateEntry(skill, next);
        SkillRecentEventStore.append(new SkillRecentEventStore.Entry(
            skill,
            "scroll_used",
            "残卷顿悟 +" + granted + " XP",
            System.currentTimeMillis()
        ));
        return ServerDataDispatch.handled(envelope.type(),
            "skill_scroll_used " + skill.wireId() + " +" + granted);
    }

    private static java.util.Set<String> mergeConsumedScroll(String scrollId) {
        java.util.LinkedHashSet<String> next = new java.util.LinkedHashSet<>(SkillSetStore.snapshot().consumedScrolls());
        next.add(scrollId);
        return java.util.Set.copyOf(next);
    }

    // ==================== JSON helpers ====================

    private static JsonPrimitive readPrimitive(JsonObject o, String f) {
        JsonElement el = o == null ? null : o.get(f);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        return el.getAsJsonPrimitive();
    }

    private static String readString(JsonObject o, String f) {
        JsonPrimitive p = readPrimitive(o, f);
        if (p == null || !p.isString()) return null;
        String s = p.getAsString();
        return s == null || s.isEmpty() ? null : s;
    }

    private static Integer readInt(JsonObject o, String f) {
        JsonPrimitive p = readPrimitive(o, f);
        if (p == null || !p.isNumber()) return null;
        String raw = p.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) return null;
        try { return Integer.parseInt(raw); } catch (NumberFormatException e) { return null; }
    }

    private static Long readLong(JsonObject o, String f) {
        JsonPrimitive p = readPrimitive(o, f);
        if (p == null || !p.isNumber()) return null;
        String raw = p.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) return null;
        try { return Long.parseLong(raw); } catch (NumberFormatException e) { return null; }
    }

    private static Boolean readBool(JsonObject o, String f) {
        JsonPrimitive p = readPrimitive(o, f);
        if (p == null || !p.isBoolean()) return null;
        return p.getAsBoolean();
    }
}
