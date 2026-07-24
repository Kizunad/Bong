package com.bong.client.audio;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonReader;

import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-fpv-cast-av-v1 P4 —— 签名音效资产管线契约（client 侧半，另半在 server
 * {@code audio::mod} 的 {@code signature_recipes_reference_registered_bong_events}）。
 *
 * <p>锁 {@code assets/bong/sounds.json} ↔ {@code assets/bong/sounds/**.ogg} 的**双向**对应：
 * <ul>
 *   <li>注册无文件 → 判红（{@link #everyRegisteredEventResolvesToExistingOgg}）；</li>
 *   <li>文件无注册（死资产）→ 判红（{@link #everyOggFileIsRegistered}）；</li>
 *   <li>8 条首批 signature 事件必须全注册（{@link #allEightSignatureEventsRegistered}）；</li>
 *   <li>client 侧 recipe（heaven_gate）引用的 {@code bong:} 事件必须在 sounds.json 注册
 *       （{@link #clientRecipeBongRefsResolveToRegisteredEvents}）——防 recipe 指向未注册事件静默无声。</li>
 * </ul>
 *
 * <p>plan 硬约束额外锁三类**错误分支 / 逐项消费**（防"只注册不消费""静默漂移"）：
 * <ul>
 *   <li>第 8 招 heaven_gate 的 client recipe 的 L0 主层必须是其 {@code bong:} 事件（{@link #heavenGateRecipesSwapL0MainLayerToTheirBongEvent}）——
 *       server 侧只 pin 了另外 7 招，这里补齐 8/8 逐项主层锁；把 recipe 主层改回 {@code minecraft:} 或挪到铺底层立即判红；</li>
 *   <li>sounds.json 重复事件键（Gson 会静默后值覆盖）——{@link #soundsJsonHasNoDuplicateEventKeys} 用 token 级流式扫描锁真实文件无重复，
 *       {@link #duplicateKeyDetectorActuallyCatchesDuplicates} 用合成 fixture 证明检测器真的能抓；</li>
 *   <li>非法命名空间前缀——{@link #everyRegisteredSoundNameUsesValidBongNamespace} 锁真实文件全 {@code bong:} 合法，
 *       {@link #illegalNamespaceNamesAreRejected} 用 fixture 证明 {@code minecraft:}/大写/空 被拒；</li>
 *   <li>committed manifest 覆盖——{@link #committedManifestAudioSubpackCoversSignatureAssets} 断言
 *       {@code client/resourcepack/manifest.json} 的 audio 子包声明覆盖了 {@code bong/sounds} 与 {@code bong/sounds.json}，
 *       且每个源 ogg 都落在子包声明路径内（防资产存在于源码却没进发布包）。</li>
 * </ul>
 *
 * <p>sound name 用「/」路径分隔（{@code bong:skill/zhenmai/sever_chain} → 文件
 * {@code sounds/skill/zhenmai/sever_chain.ogg}）；recipe 的 sound 事件 id 用「.」分隔
 * （{@code bong:skill.zhenmai.sever_chain} → sounds.json 键 {@code skill.zhenmai.sever_chain}）。
 */
class SignatureAudioContractTest {
    private static final Path RES = Path.of("src/main/resources/assets/bong");
    private static final Path SOUNDS_JSON = RES.resolve("sounds.json");
    private static final Path SOUNDS_DIR = RES.resolve("sounds");
    private static final Path CLIENT_RECIPE_DIR = RES.resolve("audio_recipes");
    /** committed 资源包 manifest（CI「Build resource pack」核验对象）。test 工作目录 = client/。 */
    private static final Path COMMITTED_MANIFEST = Path.of("resourcepack/manifest.json");

    /**
     * plan §P4 首批 8 条 signature 招的**单一 canonical 规格表**：事件键 → sounds.json sound name → ogg 相对路径。
     * 三者一一映射（事件键用「.」、sound name/文件路径用「/」）。契约测试全部从此表派生——事件↔具体 ogg
     * 逐项锁定 + 格式校验只作用于 signature 集合（不误伤未来非 signature 事件），避免 server/client 各自
     * 维护互不校验的清单。与 server signature 侧、ATTRIBUTION.md 三方对齐。
     */
    private record SignatureSpec(String event, String soundName, String oggRelPath) {}

    private static final List<SignatureSpec> SIGNATURE_SPEC = List.of(
        new SignatureSpec("skill.sword_path.heaven_gate", "bong:skill/sword_path/heaven_gate", "skill/sword_path/heaven_gate.ogg"),
        new SignatureSpec("skill.woliu.void_core", "bong:skill/woliu/void_core", "skill/woliu/void_core.ogg"),
        new SignatureSpec("skill.zhenmai.sever_chain", "bong:skill/zhenmai/sever_chain", "skill/zhenmai/sever_chain.ogg"),
        new SignatureSpec("skill.baomai.full_power_release", "bong:skill/baomai/full_power_release", "skill/baomai/full_power_release.ogg"),
        new SignatureSpec("skill.dugu.infuse_poison", "bong:skill/dugu/infuse_poison", "skill/dugu/infuse_poison.ogg"),
        new SignatureSpec("skill.tuike.shed", "bong:skill/tuike/shed", "skill/tuike/shed.ogg"),
        new SignatureSpec("skill.anqi.echo_fractal", "bong:skill/anqi/echo_fractal", "skill/anqi/echo_fractal.ogg"),
        new SignatureSpec("skill.morph.yixing", "bong:skill/morph/yixing", "skill/morph/yixing.ogg"));

    /** 从 canonical 表派生的事件键集合（供全量存在性 / 命名空间等复用）。 */
    private static final Set<String> SIGNATURE_EVENTS =
        SIGNATURE_SPEC.stream().map(SignatureSpec::event).collect(Collectors.toSet());

    /**
     * 第 8 招 heaven_gate 的 client recipe 的 **L0 主层** 逐项 pin（release + charge 尾程）。
     * plan「charge 尾程 + release」范围 + 「主层 swap」要求：签名事件必须坐 {@code layers[0]}，
     * 不是任意铺底层（挪到铺底则声音天花板没打开却假绿）。charge_2s 是压调前兆层
     * （pitch 0.72 / volume 0.4），release 是原速主音（pitch 1.0 / volume 0.9）。server 侧
     * 另 7 招同强度 pin 见 {@code audio::mod::each_server_signature_recipe_swaps_l0_main_layer_to_its_bong_event}。
     */
    private record HeavenGatePin(String recipeFile, String event, double pitch, double volume) {}

    private static final List<HeavenGatePin> HEAVEN_GATE_L0_PINS = List.of(
        new HeavenGatePin("heaven_gate_release.json", "bong:skill.sword_path.heaven_gate", 1.0, 0.9),
        new HeavenGatePin("heaven_gate_charge_2s.json", "bong:skill.sword_path.heaven_gate", 0.72, 0.4));

    private JsonObject soundsJson() throws IOException {
        assertTrue(Files.isRegularFile(SOUNDS_JSON), "sounds.json 必须提交: " + SOUNDS_JSON.toAbsolutePath());
        return JsonParser.parseString(Files.readString(SOUNDS_JSON)).getAsJsonObject();
    }

    private static String soundName(JsonElement el) {
        return el.isJsonObject() ? el.getAsJsonObject().get("name").getAsString() : el.getAsString();
    }

    @Test
    void everyRegisteredEventResolvesToExistingOgg() throws IOException {
        JsonObject root = soundsJson();
        assertFalse(root.keySet().isEmpty(), "sounds.json 不应为空——P4 至少注册 8 条 signature 事件");
        for (String event : root.keySet()) {
            JsonArray sounds = root.getAsJsonObject(event).getAsJsonArray("sounds");
            assertTrue(sounds != null && sounds.size() > 0, "事件 " + event + " 必须至少引用一个 ogg");
            for (JsonElement el : sounds) {
                String name = soundName(el);
                assertTrue(name.startsWith("bong:"),
                    "事件 " + event + " 的 sound name 期望 bong: 命名空间（自有资产），实际 " + name);
                Path ogg = SOUNDS_DIR.resolve(name.substring("bong:".length()) + ".ogg");
                assertTrue(Files.isRegularFile(ogg),
                    "事件 " + event + " 注册了 " + name + " 但 ogg 文件缺失: " + ogg
                        + " —— sounds.json 注册与实际资产漂移（注册无文件）");
                assertTrue(Files.size(ogg) > 0, "ogg 不能为空文件: " + ogg);
            }
        }
    }

    @Test
    void everyOggFileIsRegistered() throws IOException {
        JsonObject root = soundsJson();
        Set<String> referenced = new HashSet<>();
        for (String event : root.keySet()) {
            for (JsonElement el : root.getAsJsonObject(event).getAsJsonArray("sounds")) {
                referenced.add(soundName(el).substring("bong:".length()) + ".ogg");
            }
        }
        try (Stream<Path> walk = Files.walk(SOUNDS_DIR)) {
            List<Path> oggs = walk.filter(p -> p.toString().endsWith(".ogg")).collect(Collectors.toList());
            assertFalse(oggs.isEmpty(), "sounds 目录必须含 ogg 资产: " + SOUNDS_DIR.toAbsolutePath());
            for (Path ogg : oggs) {
                String rel = SOUNDS_DIR.relativize(ogg).toString().replace('\\', '/');
                assertTrue(referenced.contains(rel),
                    "ogg 文件存在但无任何 sounds.json 事件引用（死资产，白占资源包体积）: " + rel);
            }
        }
    }

    @Test
    void allEightSignatureEventsRegistered() throws IOException {
        JsonObject root = soundsJson();
        for (String ev : SIGNATURE_EVENTS) {
            assertTrue(root.has(ev), "plan §P4 首批 signature 事件缺注册: " + ev);
        }
    }

    /**
     * canonical 逐项映射：每条 signature 的 sounds.json 事件必须**精确解析**到其规格表指定的 sound name，
     * 且该 sound name 对应的 ogg 文件真实存在于规格表指定路径。防「事件挂错文件 / 路径漂移」——只查
     * 「存在某个 ogg」不够，必须是**它自己那条**。
     */
    @Test
    void eachSignatureEventResolvesToItsExactOgg() throws IOException {
        JsonObject root = soundsJson();
        for (SignatureSpec spec : SIGNATURE_SPEC) {
            assertTrue(root.has(spec.event()), "signature 事件缺注册: " + spec.event());
            JsonArray sounds = root.getAsJsonObject(spec.event()).getAsJsonArray("sounds");
            assertTrue(sounds != null && sounds.size() == 1,
                spec.event() + " 应恰好引用 1 条 sound（signature 单音源），实际 " + (sounds == null ? "null" : sounds.size()));
            assertEquals(spec.soundName(), soundName(sounds.get(0)),
                spec.event() + " 的 sound name 必须精确 == " + spec.soundName() + "（防事件挂错文件）");
            Path ogg = SOUNDS_DIR.resolve(spec.oggRelPath());
            assertTrue(Files.isRegularFile(ogg) && Files.size(ogg) > 0,
                spec.event() + " 的 ogg 必须存在于规格路径且非空: " + ogg);
        }
    }

    @Test
    void clientRecipeBongRefsResolveToRegisteredEvents() throws IOException {
        JsonObject sounds = soundsJson();
        assertTrue(Files.isDirectory(CLIENT_RECIPE_DIR), "client audio_recipes 目录必须存在: " + CLIENT_RECIPE_DIR);
        try (Stream<Path> walk = Files.walk(CLIENT_RECIPE_DIR)) {
            for (Path recipeFile : walk.filter(p -> p.toString().endsWith(".json")).collect(Collectors.toList())) {
                JsonObject recipe = JsonParser.parseString(Files.readString(recipeFile)).getAsJsonObject();
                if (!recipe.has("layers")) {
                    continue;
                }
                for (JsonElement layerEl : recipe.getAsJsonArray("layers")) {
                    String sound = layerEl.getAsJsonObject().get("sound").getAsString();
                    if (!sound.startsWith("bong:")) {
                        continue;
                    }
                    String event = sound.substring("bong:".length());
                    assertTrue(sounds.has(event),
                        "client recipe " + recipeFile.getFileName() + " 引用 bong: 事件 " + sound
                            + " 但 sounds.json 未注册该事件——运行时会静默无声");
                }
            }
        }
    }

    /**
     * 第 8 招逐项消费锁：heaven_gate 的 client recipe（charge 尾程 + release）的 **L0 主层** 必须是
     * 其 signature 事件，且 pitch/volume 符合契约。锁 {@code layers[0]}（非「任意层」）——把主层改回
     * {@code minecraft:} 或把签名挪到铺底层都会立即判红。补齐 plan 要求的 8/8 逐项主层 pin
     * （server 侧另 7 招同强度 pin 在 audio::mod）。
     */
    @Test
    void heavenGateRecipesSwapL0MainLayerToTheirBongEvent() throws IOException {
        for (HeavenGatePin pin : HEAVEN_GATE_L0_PINS) {
            Path recipeFile = CLIENT_RECIPE_DIR.resolve(pin.recipeFile());
            assertTrue(Files.isRegularFile(recipeFile),
                "heaven_gate 逐项 pin 的 recipe 缺失: " + recipeFile.toAbsolutePath());
            JsonObject recipe = JsonParser.parseString(Files.readString(recipeFile)).getAsJsonObject();
            JsonArray layers = recipe.getAsJsonArray("layers");
            assertTrue(layers != null && layers.size() > 0, pin.recipeFile() + " 必须含非空 layers");
            JsonObject l0 = layers.get(0).getAsJsonObject();
            assertEquals(pin.event(), l0.get("sound").getAsString(),
                pin.recipeFile() + " 的 **L0 主层** sound 必须 == " + pin.event()
                    + "（防签名被挪到铺底层——挪走则第 8 招签名音永不作主，声音天花板没打开）");
            assertEquals(pin.pitch(), l0.get("pitch").getAsDouble(), 1e-9,
                pin.recipeFile() + " 的 L0 pitch 契约=" + pin.pitch()
                    + "（release 原速 1.0；charge_2s 压调前兆 0.72）");
            assertEquals(pin.volume(), l0.get("volume").getAsDouble(), 1e-9,
                pin.recipeFile() + " 的 L0 volume 契约=" + pin.volume());
        }
    }

    // ---- 错误分支：重复事件键（Gson 静默后值覆盖，普通 getAsJsonObject 抓不到）----

    /** token 级流式扫描 JSON 顶层 object 的重复成员键（Gson JsonObject 会静默去重，故不能用它）。 */
    private static boolean hasDuplicateTopLevelKeys(String json) throws IOException {
        Set<String> seen = new HashSet<>();
        try (JsonReader reader = new JsonReader(new StringReader(json))) {
            reader.setLenient(true);
            reader.beginObject();
            while (reader.hasNext()) {
                String name = reader.nextName();
                if (!seen.add(name)) {
                    return true;
                }
                reader.skipValue();
            }
            reader.endObject();
        }
        return false;
    }

    @Test
    void soundsJsonHasNoDuplicateEventKeys() throws IOException {
        assertFalse(hasDuplicateTopLevelKeys(Files.readString(SOUNDS_JSON)),
            "sounds.json 顶层事件键不得重复——重复会被 Minecraft/Gson 静默后值覆盖，导致某事件悄悄丢失注册");
    }

    @Test
    void duplicateKeyDetectorActuallyCatchesDuplicates() throws IOException {
        String dup = "{\"skill.demo\":{\"sounds\":[\"bong:a\"]},\"skill.demo\":{\"sounds\":[\"bong:b\"]}}";
        assertTrue(hasDuplicateTopLevelKeys(dup),
            "重复键检测器必须能抓到合成 fixture 的重复键，否则 soundsJsonHasNoDuplicateEventKeys 是假绿");
        String ok = "{\"skill.a\":1,\"skill.b\":2}";
        assertFalse(hasDuplicateTopLevelKeys(ok), "无重复键的 fixture 不得误报");
    }

    // ---- 错误分支：非法命名空间前缀 ----

    /** 合法自有资产 sound name = {@code bong:} 命名空间 + 小写 path（字母数字与 {@code /._-}）。 */
    private static boolean isValidBongSoundName(String name) {
        if (!name.startsWith("bong:")) {
            return false;
        }
        String path = name.substring("bong:".length());
        return !path.isEmpty() && path.matches("[a-z0-9/._-]+");
    }

    @Test
    void everyRegisteredSoundNameUsesValidBongNamespace() throws IOException {
        JsonObject root = soundsJson();
        for (String event : root.keySet()) {
            for (JsonElement el : root.getAsJsonObject(event).getAsJsonArray("sounds")) {
                String name = soundName(el);
                assertTrue(isValidBongSoundName(name),
                    "事件 " + event + " 的 sound name 非法命名空间/字符: " + name
                        + "（自有签名资产必须 bong: 前缀 + 小写路径）");
            }
        }
    }

    @Test
    void illegalNamespaceNamesAreRejected() {
        assertTrue(isValidBongSoundName("bong:skill/zhenmai/sever_chain"), "合法 bong: 名应通过");
        assertFalse(isValidBongSoundName("minecraft:block.beacon.ambient"),
            "vanilla 命名空间不是自有签名资产，应被拒");
        assertFalse(isValidBongSoundName("bong:Skill/Heaven_Gate"), "大写路径应被拒（资源定位区分大小写）");
        assertFalse(isValidBongSoundName("evil:foo"), "未知命名空间应被拒");
        assertFalse(isValidBongSoundName("bong:"), "空路径应被拒");
        assertFalse(isValidBongSoundName("skill/no/namespace"), "缺命名空间应被拒");
    }

    // ---- committed manifest 覆盖：源 ogg / sounds.json 必须真进发布资源包声明路径 ----

    @Test
    void committedManifestAudioSubpackCoversSignatureAssets() throws IOException {
        assertTrue(Files.isRegularFile(COMMITTED_MANIFEST),
            "committed 资源包 manifest 必须存在: " + COMMITTED_MANIFEST.toAbsolutePath());
        JsonObject manifest = JsonParser.parseString(Files.readString(COMMITTED_MANIFEST)).getAsJsonObject();
        JsonArray packs = manifest.getAsJsonArray("packs");
        assertTrue(packs != null && packs.size() > 0, "manifest.packs 不得为空");

        JsonObject audio = null;
        for (JsonElement el : packs) {
            JsonObject pack = el.getAsJsonObject();
            if ("audio".equals(pack.get("id").getAsString())) {
                audio = pack;
                break;
            }
        }
        assertTrue(audio != null, "committed manifest 必须含 audio 子包分组");

        Set<String> audioPaths = new HashSet<>();
        for (JsonElement p : audio.getAsJsonArray("paths")) {
            audioPaths.add(p.getAsString());
        }
        assertTrue(audioPaths.contains("bong/sounds"),
            "audio 子包必须声明 bong/sounds（签名 ogg 所在），实际 " + audioPaths);
        assertTrue(audioPaths.contains("bong/sounds.json"),
            "audio 子包必须声明 bong/sounds.json（事件注册表），否则独立加载 audio 子包时 bong: 事件无注册 → 静默无声；实际 "
                + audioPaths);

        // 每个源 ogg 的相对路径必须落在某个声明前缀下（bong/sounds/**），证明它会被打进发布包。
        try (Stream<Path> walk = Files.walk(SOUNDS_DIR)) {
            List<Path> oggs = walk.filter(p -> p.toString().endsWith(".ogg")).collect(Collectors.toList());
            assertFalse(oggs.isEmpty(), "sounds 目录必须含 ogg 资产");
            for (Path ogg : oggs) {
                String rel = "bong/sounds/" + SOUNDS_DIR.relativize(ogg).toString().replace('\\', '/');
                boolean covered = audioPaths.stream().anyMatch(prefix -> rel.equals(prefix) || rel.startsWith(prefix + "/"));
                assertTrue(covered,
                    "签名 ogg " + rel + " 未被 committed manifest 的 audio 子包任一声明路径覆盖 → 不会进发布包；audioPaths="
                        + audioPaths);
            }
        }
    }

    // ---- 音频格式契约：ffmpeg 完整解码 + ffprobe 精确采样数（统一谓词，正负向共用）----
    //
    // plan §P4「mono, 44.1kHz，短样本 ≤3s」。只读元数据不够（损坏 packet 的伪 Vorbis 元数据仍合法），
    // 故 ① `ffmpeg -f null` 完整解码每个 packet（退出码非 0 = 损坏/不可解码）② `ffprobe -count_samples`
    // 读**精确采样数**，严格 ≤132300（= 3s @ 44.1kHz，不用浮点容差）。正向真资产与负向 fixture（ffmpeg
    // 现场 lavfi 合成的 stereo/48k/超长/非 Vorbis）**共用同一 validateSignatureAudio 谓词**，防测试谓词与门禁
    // 漂移。工具/编码器缺失时 Assumptions 跳过而非误红——skip-if-absent 优于因缺工具破红 CI；标准 CI
    // （ubuntu-latest 自带 ffmpeg）与本地均可用。校验只作用于 SIGNATURE_SPEC（不误伤未来非 signature 事件）。

    private static final int SIGNATURE_SAMPLE_RATE = 44100;
    /** 3s @ 44.1kHz 的精确采样上界（严格 ≤3s，无浮点容差）。 */
    private static final long MAX_SIGNATURE_SAMPLES = SIGNATURE_SAMPLE_RATE * 3L;

    private record ProcResult(int exit, String out) {}

    private static ProcResult run(String... cmd) throws IOException, InterruptedException {
        Process p = new ProcessBuilder(cmd).redirectErrorStream(true).start();
        String out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        return new ProcResult(p.waitFor(), out);
    }

    private static boolean toolAvailable(String tool) {
        try {
            return run(tool, "-version").exit() == 0;
        } catch (IOException e) {
            return false;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    /**
     * 统一签名音频校验谓词（正向真资产 + 负向 fixture 共用，防漂移）：
     * ① `ffmpeg -f null` 完整解码成功；② `ffprobe -count_samples` 精确 codec=vorbis / mono / 44.1kHz /
     * 0 < 采样数 ≤ 132300。返回问题列表（空 = 合格）。
     */
    private static List<String> validateSignatureAudio(Path file) throws IOException, InterruptedException {
        List<String> problems = new ArrayList<>();
        ProcResult decode = run("ffmpeg", "-v", "error", "-nostdin",
            "-i", file.toString(), "-map", "0:a:0", "-f", "null", "-");
        if (decode.exit() != 0) {
            // 完整解码失败（损坏 packet / 无音频流 / 不可解码）——元数据无意义，直接判红
            problems.add("完整解码失败（ffmpeg -f null exit=" + decode.exit() + "）: " + decode.out().strip());
            return problems;
        }
        // duration_ts = 流时长（time_base 单位）；44.1kHz Vorbis 的 time_base = 1/44100，故 duration_ts
        // 即**精确采样数**（3.0s → 132300），比 float duration 严格、无浮点容差。
        ProcResult probe = run("ffprobe", "-v", "error", "-select_streams", "a:0",
            "-show_entries", "stream=codec_name,channels,sample_rate,duration_ts",
            "-of", "default=noprint_wrappers=1:nokey=0", file.toString());
        String codec = "";
        int channels = 0;
        int sampleRate = 0;
        long samples = -1;
        for (String line : probe.out().split("\\R")) {
            int eq = line.indexOf('=');
            if (eq < 0) {
                continue;
            }
            String k = line.substring(0, eq).trim();
            String v = line.substring(eq + 1).trim();
            try {
                switch (k) {
                    case "codec_name" -> codec = v;
                    case "channels" -> channels = Integer.parseInt(v);
                    case "sample_rate" -> sampleRate = Integer.parseInt(v);
                    case "duration_ts" -> samples = Long.parseLong(v);
                    default -> { }
                }
            } catch (NumberFormatException ignored) {
                // "N/A" 等非数值字段保持默认，由下方断言判红
            }
        }
        if (!"vorbis".equals(codec)) {
            problems.add("codec 非 vorbis: " + codec);
        }
        if (channels != 1) {
            problems.add("非 mono: 声道=" + channels);
        }
        if (sampleRate != SIGNATURE_SAMPLE_RATE) {
            problems.add("非 44.1kHz: " + sampleRate);
        }
        if (samples <= 0) {
            problems.add("无音频采样: duration_ts=" + samples);
        } else if (samples > MAX_SIGNATURE_SAMPLES) {
            problems.add("超 3s: " + samples + " > " + MAX_SIGNATURE_SAMPLES + " 采样");
        }
        return problems;
    }

    @Test
    void everySignatureOggFullyDecodesAsMonoVorbis44kUnderThreeSeconds() throws Exception {
        Assumptions.assumeTrue(toolAvailable("ffmpeg") && toolAvailable("ffprobe"),
            "ffmpeg/ffprobe 不可用——跳过真解码格式门禁（标准 CI/本地应自带 ffmpeg）");
        for (SignatureSpec spec : SIGNATURE_SPEC) {
            Path ogg = SOUNDS_DIR.resolve(spec.oggRelPath());
            assertTrue(Files.isRegularFile(ogg), spec.event() + " 的 ogg 缺失: " + ogg);
            List<String> problems = validateSignatureAudio(ogg);
            assertTrue(problems.isEmpty(),
                spec.event() + " 的 ogg 不满足签名格式契约（完整解码 + mono/44.1k/≤3s）: " + problems + " @ " + ogg);
        }
    }

    /** ffmpeg lavfi 合成一个测试音到 dir/name（指定声道数与编码器），返回其路径。失败即断言错。 */
    private static Path synth(Path dir, String name, String lavfi, int channels, String codec)
            throws IOException, InterruptedException {
        Path out = dir.resolve(name);
        ProcResult r = run("ffmpeg", "-v", "error", "-nostdin", "-y",
            "-f", "lavfi", "-i", lavfi, "-ac", Integer.toString(channels), "-c:a", codec, out.toString());
        assertEquals(0, r.exit(), "合成 fixture " + name + " 失败（ffmpeg 缺 " + codec + " 编码器?）: " + r.out().strip());
        return out;
    }

    private static boolean canSynthesizeVorbisAndOpus() {
        try {
            Path probe = Files.createTempDirectory("bong-synth-probe-");
            try {
                synth(probe, "v.ogg", "sine=frequency=440:duration=1:sample_rate=44100", 1, "libvorbis");
                synth(probe, "o.ogg", "sine=frequency=440:duration=1:sample_rate=48000", 1, "libopus");
                return true;
            } finally {
                deleteRecursively(probe);
            }
        } catch (Exception e) {
            return false;
        }
    }

    private static void deleteRecursively(Path dir) throws IOException {
        try (Stream<Path> walk = Files.walk(dir)) {
            walk.sorted(Comparator.reverseOrder()).forEach(p -> {
                try {
                    Files.deleteIfExists(p);
                } catch (IOException ignored) {
                    // best-effort 清理
                }
            });
        }
    }

    @Test
    void signatureValidatorRejectsRealOutOfSpecFixtures() throws Exception {
        Assumptions.assumeTrue(toolAvailable("ffmpeg") && toolAvailable("ffprobe") && canSynthesizeVorbisAndOpus(),
            "ffmpeg 缺 libvorbis/libopus 编码器——跳过合成 fixture 错误分支门禁");
        Path dir = Files.createTempDirectory("bong-audio-fixtures-");
        try {
            // 控制组：合成的合法 mono/44.1k/1s Vorbis 走同一谓词应通过（证明合成路径本身能过）
            Path ok = synth(dir, "ok.ogg", "sine=frequency=440:duration=1:sample_rate=44100", 1, "libvorbis");
            assertTrue(validateSignatureAudio(ok).isEmpty(), "合成的合法 mono/44.1k/1s Vorbis 应通过谓词");

            // 负向：每个只破坏一项，全部走同一 validateSignatureAudio（防谓词漂移）
            Path stereo = synth(dir, "stereo.ogg", "sine=frequency=440:duration=1:sample_rate=44100", 2, "libvorbis");
            assertFalse(validateSignatureAudio(stereo).isEmpty(), "stereo Vorbis 必须被谓词判红");

            Path hz48 = synth(dir, "48k.ogg", "sine=frequency=440:duration=1:sample_rate=48000", 1, "libvorbis");
            assertFalse(validateSignatureAudio(hz48).isEmpty(), "48kHz Vorbis 必须被判红");

            Path long4s = synth(dir, "long.ogg", "sine=frequency=440:duration=4:sample_rate=44100", 1, "libvorbis");
            assertFalse(validateSignatureAudio(long4s).isEmpty(), ">3s Vorbis 必须被判红");

            Path opus = synth(dir, "opus.ogg", "sine=frequency=440:duration=1:sample_rate=48000", 1, "libopus");
            assertFalse(validateSignatureAudio(opus).isEmpty(), "非 Vorbis codec（Opus）必须被判红");

            // 不可解码：非音频字节 → ffmpeg 完整解码失败（找不到音频流）
            Path garbage = dir.resolve("garbage.ogg");
            Files.write(garbage, "NOT_A_REAL_AUDIO_STREAM".getBytes(StandardCharsets.UTF_8));
            assertFalse(validateSignatureAudio(garbage).isEmpty(), "非解码文件必须被判红（完整解码失败）");
        } finally {
            deleteRecursively(dir);
        }
    }
}
