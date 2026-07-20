package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

/**
 * plan §P5.1 粒子 spec 表 ↔ 三个 player 生产常量的**逐格对拍**
 * —— plan-skill-anim-fidelity-v1（review r1 一致性要求）。
 *
 * <p><b>为什么要解析文档</b>：review r1 的核心教训是 spec 与实现各自漂移——文档写「穴位点静止 /
 * lifetime 20t」，实现给了上浮 0.008 和 lifetime+6，两边都"自洽"却互相矛盾，而当时的测试两边
 * 都没测。修法不是"这次对齐一下"，而是让**任一侧改动而另一侧没跟就撞红**：本测试直接消费
 * plan markdown 表格里的数值，与 {@code Form} 枚举字段对拍。
 *
 * <p>解析而非硬抄数值是关键——如果这里再抄一份常量，就成了第三份会漂的副本。
 *
 * <p>plan 归档后会 {@code git mv} 进 {@code docs/finished_plans/}，故按候选路径依次查找；
 * 两处都找不到才失败（那意味着 spec 被移走或删除，本身就该撞红而不是静默跳过）。
 */
public class SkillParticleSpecDocTest {

    /** gradle test 默认 cwd = client/，plan 在仓库根 docs/ 下。归档后落 finished_plans/。 */
    private static final List<Path> PLAN_CANDIDATES = List.of(
        Path.of("..", "docs", "plan-skill-anim-fidelity-v1.md"),
        Path.of("..", "docs", "finished_plans", "plan-skill-anim-fidelity-v1.md")
    );

    private static final double EPS = 1e-9;

    /** event_id → （列名 → 单元格原文）。 */
    private static Map<String, Map<String, String>> specRows() throws IOException {
        Path plan = PLAN_CANDIDATES.stream()
            .filter(Files::isRegularFile)
            .findFirst()
            .orElseGet(() -> fail("找不到 plan spec 文档，候选路径：" + PLAN_CANDIDATES.stream()
                .map(p -> p.toAbsolutePath().toString()).toList()));

        Map<String, Map<String, String>> rows = new LinkedHashMap<>();
        List<String> block = new ArrayList<>();
        for (String line : Files.readAllLines(plan)) {
            String trimmed = line.strip();
            if (trimmed.startsWith("|")) {
                block.add(trimmed);
                continue;
            }
            harvest(block, rows);
            block.clear();
        }
        harvest(block, rows);
        return rows;
    }

    /**
     * 把一段连续的 markdown 表格行（表头 + 分隔行 + 数据行）收进 rows。
     *
     * <p>只收 §P5.1 的三张粒子 spec 表——用「运动形态」列作判别。plan 里另有两张表也以
     * {@code bong:} 开头：§P5.1 ⑤ 旧借用 id 去向表、§P5.2 接线矩阵（后者首列是招式 id 不是
     * event_id，本就不会命中）。把它们一并收进来会让「spec 表不得有僵尸行」的判据误伤。
     */
    private static void harvest(List<String> block, Map<String, Map<String, String>> rows) {
        if (block.size() < 3 || !block.get(1).matches("\\|[\\s\\-:|]+\\|")) {
            return;
        }
        List<String> header = cells(block.get(0));
        if (!header.contains("运动形态")) {
            return;
        }
        for (String line : block.subList(2, block.size())) {
            List<String> values = cells(line);
            if (values.isEmpty()) {
                continue;
            }
            String key = values.get(0).replace("`", "").strip();
            if (!key.startsWith("bong:")) {
                continue;
            }
            Map<String, String> row = new LinkedHashMap<>();
            for (int i = 0; i < header.size() && i < values.size(); i++) {
                row.put(header.get(i).strip(), values.get(i).strip());
            }
            rows.put(key, row);
        }
    }

    private static List<String> cells(String line) {
        String body = line.strip();
        body = body.substring(1, body.length() - (body.endsWith("|") ? 1 : 0));
        List<String> out = new ArrayList<>();
        for (String cell : body.split("\\|", -1)) {
            out.add(cell.strip());
        }
        return out;
    }

    // ─── 单元格取值 helper ──────────────────────────────────────────────────────

    private static String cell(Map<String, Map<String, String>> rows, String id, String column) {
        Map<String, String> row = rows.get(id);
        assertNotNull(row, "plan §P5.1 spec 表里找不到 " + id + " 这一行");
        String value = row.get(column);
        assertNotNull(value,
            id + " 行缺列「" + column + "」——实际列名：" + row.keySet());
        return value;
    }

    /** 抽出单元格里所有十进制数（负号计入）。用于 `0.30 + 0.45×strength` 这类复合格。 */
    private static List<Double> numbers(String cellText) {
        List<Double> out = new ArrayList<>();
        Matcher matcher = Pattern.compile("-?\\d+(?:\\.\\d+)?").matcher(cellText);
        while (matcher.find()) {
            out.add(Double.parseDouble(matcher.group()));
        }
        return out;
    }

    private static double number(Map<String, Map<String, String>> rows, String id, String column) {
        String text = cell(rows, id, column);
        List<Double> found = numbers(text);
        assertEquals(1, found.size(),
            id + " 的「" + column + "」格应恰含 1 个数值，实际「" + text + "」解析出 " + found);
        return found.get(0);
    }

    private static int hex(Map<String, Map<String, String>> rows, String id) {
        String text = cell(rows, id, "颜色 hex");
        Matcher matcher = Pattern.compile("#([0-9A-Fa-f]{6})").matcher(text);
        assertTrue(matcher.find(), id + " 的颜色格里没有 #RRGGBB：" + text);
        return Integer.parseInt(matcher.group(1), 16);
    }

    // ─── ① zhenmai 5 招 ─────────────────────────────────────────────────────────

    @Test
    void zhenmaiSpecTableMatchesFormConstants() throws IOException {
        Map<String, Map<String, String>> rows = specRows();

        for (ZhenmaiPulsePlayer.Form form : ZhenmaiPulsePlayer.Form.values()) {
            String id = form.eventId.toString();

            assertEquals(form.motion.name(), cell(rows, id, "运动形态"),
                id + " 的运动形态：spec 与 Form.motion 不一致");
            assertEquals(form.defaultPulses, (int) number(rows, id, "脉冲数"),
                id + " 的脉冲数漂了");
            assertEquals(form.acupoints, (int) number(rows, id, "穴位点数"),
                id + " 的穴位点数漂了");
            assertEquals(ZhenmaiPulsePlayer.DEFAULT_LIFETIME_TICKS,
                (int) number(rows, id, "lifetime"),
                id + " 的 lifetime 漂了（真脉 5 招统一 20t，与 server duration_ticks 一致）");
            assertEquals(form.speed, number(rows, id, "主速度 格/t"), EPS,
                id + " 的主速度漂了");
            assertEquals(form.pulseVertical, number(rows, id, "脉冲垂直 格/t"), EPS,
                id + " 的脉冲垂直速度漂了");
            assertEquals(form.acupointDrift, number(rows, id, "穴位点漂移 格/t"), EPS,
                id + " 的穴位点漂移漂了——review r1 #1 正是栽在这一格上");
            assertEquals(form.radius, number(rows, id, "半径 格"), EPS,
                id + " 的半径漂了");
            assertEquals(form.fallbackRgb, hex(rows, id), id + " 的兜底色漂了");
        }
    }

    // ─── ② burst_meridian 3 招 ──────────────────────────────────────────────────

    @Test
    void burstSpecTableMatchesFormConstants() throws IOException {
        Map<String, Map<String, String>> rows = specRows();

        for (BurstMeridianFamilyPlayer.Form form : BurstMeridianFamilyPlayer.Form.values()) {
            String id = form.eventId.toString();

            assertEquals(form.motion.name(), cell(rows, id, "运动形态"),
                id + " 的运动形态：spec 与 Form.motion 不一致");
            assertEquals(form.defaultCount, (int) number(rows, id, "数量"),
                id + " 的默认数量漂了");
            assertEquals(form.minCount, (int) number(rows, id, "count 下限"),
                id + " 的 count 下限漂了");
            assertEquals(form.defaultLifetime, (int) number(rows, id, "lifetime"),
                id + " 的缺省 lifetime 漂了");
            assertEquals(form.speed, number(rows, id, "主速度 格/t"), EPS,
                id + " 的主速度漂了");
            assertEquals(form.verticalSpeed, number(rows, id, "垂直速度 格/t"), EPS,
                id + " 的垂直速度漂了");
            assertEquals(BurstMeridianFamilyPlayer.FAMILY_RGB, hex(rows, id),
                id + " 的家族识别色漂了");

            // 半径：撞击环是 `0.30 + 0.45×strength` 两个数，护体是单值，残影「不适用」。
            List<Double> radius = numbers(cell(rows, id, "半径 格"));
            if (radius.isEmpty()) {
                assertEquals(0.0, form.radiusBase, EPS,
                    id + " 的 spec 半径格写「不适用」，实现却有非零 radiusBase");
                assertEquals(0.0, form.radiusStrength, EPS);
            } else if (radius.size() == 1) {
                assertEquals(form.radiusBase, radius.get(0), EPS, id + " 的半径漂了");
                assertEquals(0.0, form.radiusStrength, EPS,
                    id + " 的 spec 半径是单值，实现却带了 strength 系数");
            } else {
                assertEquals(form.radiusBase, radius.get(0), EPS, id + " 的半径基值漂了");
                assertEquals(form.radiusStrength, radius.get(1), EPS,
                    id + " 的半径 strength 系数漂了");
            }

            // 自旋：只有贴地符圈用得上。
            List<Double> spin = numbers(cell(rows, id, "自旋 rad/t"));
            assertEquals(form.spinRate, spin.isEmpty() ? 0.0 : spin.get(0), EPS,
                id + " 的自旋速率漂了");
        }
    }

    // ─── ③ npc 3 招 ─────────────────────────────────────────────────────────────

    @Test
    void npcSpecTableMatchesFormConstants() throws IOException {
        Map<String, Map<String, String>> rows = specRows();

        for (NpcSkillAuraPlayer.Form form : NpcSkillAuraPlayer.Form.values()) {
            String id = form.eventId.toString();

            assertEquals(form.motion.name(), cell(rows, id, "运动形态"),
                id + " 的运动形态：spec 与 Form.motion 不一致");
            assertEquals(NpcSkillAuraPlayer.DEFAULT_COUNT, (int) number(rows, id, "数量"),
                id + " 的默认数量漂了（应与 server count: Some(12) 一致）");
            assertEquals(NpcSkillAuraPlayer.DEFAULT_LIFETIME_TICKS,
                (int) number(rows, id, "lifetime"),
                id + " 的 lifetime 漂了（应与 server duration_ticks: Some(40) 一致）");
            assertEquals(form.speed, number(rows, id, "主速度 格/t"), EPS,
                id + " 的主速度漂了");
            assertEquals(form.radius, number(rows, id, "半径 格"), EPS,
                id + " 的半径漂了");
            assertEquals(form.verticalSpeed, number(rows, id, "垂直速度 格/t"), EPS,
                id + " 的垂直速度漂了");
            assertEquals(form.fallbackRgb, hex(rows, id), id + " 的配色漂了");
        }
    }

    // ─── 覆盖完整性：spec 表不得漏招，也不得留下已删招的僵尸行 ──────────────────

    @Test
    void specTableCoversExactlyTheElevenWiredSkills() throws IOException {
        Map<String, Map<String, String>> rows = specRows();

        List<String> wired = new ArrayList<>();
        for (net.minecraft.util.Identifier id : ZhenmaiPulsePlayer.EVENT_IDS) {
            wired.add(id.toString());
        }
        for (net.minecraft.util.Identifier id : BurstMeridianFamilyPlayer.EVENT_IDS) {
            wired.add(id.toString());
        }
        for (net.minecraft.util.Identifier id : NpcSkillAuraPlayer.EVENT_IDS) {
            wired.add(id.toString());
        }
        assertEquals(11, wired.size(), "P5 共 11 招接线（真脉 5 + 爆脉 3 + NPC 3）");

        for (String id : wired) {
            assertTrue(rows.containsKey(id),
                id + " 有实现却没在 plan §P5.1 spec 表里——视听规格缺失");
        }

        // spec 表里不得留下没有实现的僵尸行（含崩拳本尊——它归 BurstMeridianBengQuanPlayer，
        // 只该出现在 ⑤ 旧借用去向表，不该混进 P5.1 spec 表）。
        assertEquals(wired.size(), rows.size(),
            "P5.1 spec 表的行数与实际接线招数不符：spec 有 " + rows.keySet()
                + "，实现有 " + wired);
        for (String id : rows.keySet()) {
            assertTrue(wired.contains(id),
                id + " 出现在 P5.1 spec 表却没有对应实现——spec 与实现脱节");
        }
    }
}
