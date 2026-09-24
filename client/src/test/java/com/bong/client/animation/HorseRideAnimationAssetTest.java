package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 骑马坐姿资源 pin 测试（生成器：{@code client/tools/gen_horse_ride.py}）。
 *
 * <p>锁三件事，都是这套姿势**唯一会坏在**的地方：
 * <ol>
 *   <li><b>两副坐姿必须看得出区别</b>——三档鞍合成两副，合并的理由是"量下来一样"；
 *       那留下的这两副就必须真的不一样，否则合并合过头了。判据落在屈膝角上：
 *       没有镫的腿是垂着的，有镫的膝盖折起来，差着三十几度。</li>
 *   <li><b>大腿俯仰不许过 40°</b>——MC 没有 IK，过了大腿就和胯脱开
 *       （docs/player-animation-conventions.md）。折角该由 bend 担。</li>
 *   <li><b>循环两端逐轴同值</b>——PlayerAnimator 的 Axis.findAfter 会在循环处虚构一帧
 *       指向 defaultValue，只在 tick 0 给值的轴会一路衰减回零（半途 T-pose）。</li>
 * </ol>
 */
public class HorseRideAnimationAssetTest {
    private static final Path ROOT = Path.of("src/main/resources/assets/bong/player_animation");
    private static final String BAREBACK = "horse_ride_bareback";
    private static final String STIRRUP = "horse_ride_stirrup";
    /** 两副坐姿的屈膝角至少要差这么多度，否则"合并成两副"就合过头了。 */
    private static final double MIN_BEND_GAP_DEG = 20.0;
    /** MC 无 IK 时大腿俯仰的硬上限（度）。 */
    private static final double PITCH_CAP_DEG = 40.0;

    @Test
    void bothSeatsExistAndAreValidV3() throws IOException {
        for (String id : new String[] {BAREBACK, STIRRUP}) {
            Path path = ROOT.resolve(id + ".json");
            assertTrue(Files.isRegularFile(path), "缺少骑马坐姿资源: " + path);
            JsonObject root = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
            assertEquals(3, root.get("version").getAsInt(), id + " 必须是 Emotecraft v3 JSON");
            assertEquals(id, root.get("name").getAsString(), id + " 文件名必须和 JSON name 一致");
            JsonObject emote = root.getAsJsonObject("emote");
            assertTrue(emote.get("isLoop").getAsBoolean(), id + " 坐姿是常驻的，必须循环");
            assertTrue(emote.get("endTick").getAsInt() > 0, id + " endTick 必须为正");
            assertTrue(emote.getAsJsonArray("moves").size() > 0, id + " 必须含关键帧");
        }
    }

    @Test
    void twoSeatsAreVisiblyDifferent() throws IOException {
        double bare = Math.toDegrees(axisAt(BAREBACK, 0, "rightLeg", "bend"));
        double stir = Math.toDegrees(axisAt(STIRRUP, 0, "rightLeg", "bend"));
        assertTrue(Math.abs(stir - bare) >= MIN_BEND_GAP_DEG,
            "两副坐姿的屈膝角只差 " + Math.abs(stir - bare) + "°（下限 " + MIN_BEND_GAP_DEG
                + "°）——无镫那副该是腿垂着，踩镫那副该把膝盖折起来，差不出来就不该分两副");
        assertTrue(bare < stir, "无镫那副的膝盖应当比踩镫那副更直（脚没处踩）");
    }

    @Test
    void thighPitchStaysWithinNoIkCap() throws IOException {
        for (String id : new String[] {BAREBACK, STIRRUP}) {
            for (String leg : new String[] {"leftLeg", "rightLeg"}) {
                double deg = Math.abs(Math.toDegrees(axisAt(id, 0, leg, "pitch")));
                assertTrue(deg <= PITCH_CAP_DEG,
                    id + "/" + leg + " 大腿俯仰 " + deg + "° 超过 " + PITCH_CAP_DEG
                        + "°——MC 没有 IK，大腿会和胯脱开；折角该由 bend 担");
            }
        }
    }

    @Test
    void legsAreMirrored() throws IOException {
        for (String id : new String[] {BAREBACK, STIRRUP}) {
            for (String axis : new String[] {"pitch", "bend", "roll"}) {
                double l = axisAt(id, 0, "leftLeg", axis);
                double r = axisAt(id, 0, "rightLeg", axis);
                double want = axis.equals("roll") ? -l : l;  // 只有外张左右反号
                assertEquals(want, r, 1e-7,
                    id + " 两腿的 " + axis + " 不是镜像——镫是一对的");
            }
        }
    }

    @Test
    void loopClosesOnEveryUsedAxis() throws IOException {
        for (String id : new String[] {BAREBACK, STIRRUP}) {
            JsonObject emote = JsonParser.parseString(Files.readString(ROOT.resolve(id + ".json")))
                .getAsJsonObject().getAsJsonObject("emote");
            int endTick = emote.get("endTick").getAsInt();
            Map<String, Double> first = valuesAt(emote.getAsJsonArray("moves"), 0);
            Map<String, Double> last = valuesAt(emote.getAsJsonArray("moves"), endTick);
            Set<String> keys = new HashSet<>(first.keySet());
            keys.addAll(last.keySet());
            for (String key : keys) {
                Double a = first.get(key);
                Double b = last.get(key);
                assertTrue(a != null && b != null,
                    id + " 的 " + key + " 只在循环的一端有关键帧——另一端会被插值回 defaultValue");
                assertEquals(a, b, 1e-7,
                    id + " 的 " + key + " 在循环两端不等（" + a + " vs " + b + "）——每轮会咯噔一下");
            }
        }
    }

    private static Map<String, Double> valuesAt(JsonArray moves, int tick) {
        Map<String, Double> out = new LinkedHashMap<>();
        for (int i = 0; i < moves.size(); i++) {
            JsonObject move = moves.get(i).getAsJsonObject();
            if (move.get("tick").getAsInt() != tick) {
                continue;
            }
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing")) {
                    continue;
                }
                JsonObject axes = move.getAsJsonObject(part);
                for (String axis : axes.keySet()) {
                    out.put(part + "." + axis, axes.get(axis).getAsDouble());
                }
            }
        }
        return out;
    }

    private static double axisAt(String id, int tick, String part, String axis) throws IOException {
        JsonObject emote = JsonParser.parseString(Files.readString(ROOT.resolve(id + ".json")))
            .getAsJsonObject().getAsJsonObject("emote");
        Double v = new HashMap<>(valuesAt(emote.getAsJsonArray("moves"), tick)).get(part + "." + axis);
        assertTrue(v != null, id + " 缺 " + part + "." + axis + " 关键帧");
        return v;
    }
}
