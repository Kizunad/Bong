package com.bong.client.death;

import com.bong.client.combat.store.TerminationSummary;

/** 四种域外口吻的预生成低语。只作叙事点缀，不宣称玩家触发了隐藏机制。 */
public record OtherworldVerdict(String voice, String words, int accent) {
    private static final String[][] LINES = {
        {"止于此处。\n余下的，尚不足记。", "这些痕迹留着。\n名字就不必了。", "比先前多走了一段。\n再远些，才算数。"},
        {"已经很好了。\n把舍不得的，也放下吧。", "别怕。那里留下的伤，\n这里不会替你记住。", "睡吧。\n这回，就不问你带回了什么。"},
        {"就这些？\n里面分明还有。", "壳已碎了。\n你寻到的那一点呢？", "那边快空了。\n偏偏你也空着回来。"},
        {"差一点。\n你竟真把那里，当成了归处。", "你替他们护住了什么？\n……倒也有趣。", "又学会了一个名字。\n这次打算留多久？"}
    };
    private static final String[] VOICES = {"远处，一声点记", "像是有人轻声哄你", "有什么仍未餍足", "一声不合时宜的轻笑"};
    private static final int[] COLORS = {0xFF96ADA0, 0xFFBFA78A, 0xFFC58978, 0xFFAEADB9};

    public static OtherworldVerdict choose(TerminationSummary summary, int seed) {
        int voice = Math.floorMod(seed, LINES.length);
        int line = summary.deathCount() >= 4 ? 2 : summary.meridiansOpen() != null && summary.meridiansOpen() > 0 ? 1 : 0;
        return new OtherworldVerdict(VOICES[voice], LINES[voice][line], COLORS[voice]);
    }
}
