package com.bong.client.audio;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonReader;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
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

    /** plan §P4 首批 8 条 signature 招（与 server signature 侧、ATTRIBUTION.md 三方对齐）。 */
    private static final Set<String> SIGNATURE_EVENTS = Set.of(
        "skill.sword_path.heaven_gate",
        "skill.woliu.void_core",
        "skill.zhenmai.sever_chain",
        "skill.baomai.full_power_release",
        "skill.dugu.infuse_poison",
        "skill.tuike.shed",
        "skill.anqi.echo_fractal",
        "skill.morph.yixing");

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

    // ---- 音频格式契约：机器校验 Ogg Vorbis codec/channels/sample_rate/duration + 容器完整性 ----
    //
    // plan §P4 音源规格写明「mono, 44.1kHz，短样本 ≤3s」。仅校验文件存在+非空挡不住换源引入的
    // stereo / 48kHz / 超长 / 伪 OGG——这里解析 Ogg Vorbis 容器逐项机器 pin。probe 只依赖标准库
    // （不引第三方解码器，避免 CI 环境依赖）；它是**结构探针不是完整解码器**，故除 codec/声道/采样率/
    // 时长外还校验三项完整性以挡「有效魔数前缀但截断/无音频」的伪文件：① identification header 完整
    // 30 字节且 framing bit 置位 ② 末页带 EOS 标志（header_type & 0x04）③ 末页 granule>0（=确有 PCM
    // 采样）。真资产由 ffmpeg 管线保证可解码，本探针在测试期兜住结构层伪造。

    private static final int SIGNATURE_SAMPLE_RATE = 44100;
    private static final double SIGNATURE_MAX_SECONDS = 3.0;
    // 采样对齐的时长上界容差：3.0s 正好 = 132300 granule；编码 padding 可能多出个别采样，放 ~5ms
    // 容差既容忍边界又能判死 4s 级越界。
    private static final double SIGNATURE_DURATION_EPS = 0.005;
    /** Vorbis identification header 固定 30 字节（含末尾 framing flag）。 */
    private static final int VORBIS_ID_HEADER_LEN = 30;

    private record OggVorbisProbe(
        boolean vorbis, int channels, int sampleRate, double durationSeconds, boolean structurallyComplete) {}

    /** 末页扫描结果：granule_position（Vorbis 里 = 累计 PCM 采样数）+ header_type（bit2=EOS）。 */
    private record LastPage(long granule, int headerType) {}

    private static boolean startsWithOggS(byte[] data, int off) {
        return off + 4 <= data.length
            && data[off] == 'O' && data[off + 1] == 'g' && data[off + 2] == 'g' && data[off + 3] == 'S';
    }

    private static int readLeU32(byte[] b, int off) {
        return (b[off] & 0xFF) | ((b[off + 1] & 0xFF) << 8) | ((b[off + 2] & 0xFF) << 16) | ((b[off + 3] & 0xFF) << 24);
    }

    private static long readLeU64(byte[] b, int off) {
        long v = 0;
        for (int i = 0; i < 8; i++) {
            v |= ((long) (b[off + i] & 0xFF)) << (8 * i);
        }
        return v;
    }

    /**
     * 逐页扫描到最后一个 Ogg 页，返回其 granule + header_type（header_type@页偏移+5）。
     * 逐页跳转：body 长度 = segment_table 各字节之和（含 255 lacing）。截断/越界即停在已见的最后一页。
     */
    private static LastPage scanLastPage(byte[] data) {
        long granule = 0;
        int headerType = 0;
        int i = 0;
        while (i + 27 <= data.length) {
            if (!startsWithOggS(data, i)) {
                i++; // well-formed 文件页对齐，resync 兜底
                continue;
            }
            int segs = data[i + 26] & 0xFF;
            int segTableEnd = i + 27 + segs;
            if (segTableEnd > data.length) {
                break; // segment table 被截断 → 本页不完整，不采信其 granule/EOS
            }
            int bodyLen = 0;
            for (int s = 0; s < segs; s++) {
                bodyLen += data[i + 27 + s] & 0xFF;
            }
            if (segTableEnd + bodyLen > data.length) {
                break; // page body 被截断 → 文件不完整
            }
            headerType = data[i + 5] & 0xFF;
            granule = readLeU64(data, i + 6);
            i = segTableEnd + bodyLen;
        }
        return new LastPage(granule, headerType);
    }

    /**
     * 解析 Ogg Vorbis：首页 identification header 取 codec/channels/sample_rate，末页 granule/sample_rate
     * 得时长。非 Ogg 或非 Vorbis identification header → vorbis=false。structurallyComplete = id header 完整
     * 30 字节且 framing bit 置位 && 末页带 EOS && granule>0（挡截断/无音频伪文件）。
     */
    private static OggVorbisProbe probeOggVorbis(byte[] data) {
        if (!startsWithOggS(data, 0) || data.length < 28) {
            return new OggVorbisProbe(false, 0, 0, 0.0, false);
        }
        int pageSegments = data[26] & 0xFF;
        int packetStart = 27 + pageSegments;
        // identification header: 0x01 "vorbis" version(4) channels(1) sample_rate(4) ... blocksize(1) framing(1)
        if (packetStart + 16 > data.length
            || (data[packetStart] & 0xFF) != 0x01
            || data[packetStart + 1] != 'v' || data[packetStart + 2] != 'o' || data[packetStart + 3] != 'r'
            || data[packetStart + 4] != 'b' || data[packetStart + 5] != 'i' || data[packetStart + 6] != 's') {
            return new OggVorbisProbe(false, 0, 0, 0.0, false);
        }
        int channels = data[packetStart + 11] & 0xFF;
        int sampleRate = readLeU32(data, packetStart + 12);
        // framing flag @ id header 偏移 29（bit0 必须为 1）；要求 header 完整 30 字节。
        boolean framingOk = packetStart + VORBIS_ID_HEADER_LEN <= data.length
            && (data[packetStart + 29] & 0x01) == 0x01;
        LastPage last = scanLastPage(data);
        boolean eos = (last.headerType() & 0x04) != 0;
        boolean structurallyComplete = framingOk && eos && last.granule() > 0;
        double duration = sampleRate > 0 ? (double) last.granule() / sampleRate : 0.0;
        return new OggVorbisProbe(true, channels, sampleRate, duration, structurallyComplete);
    }

    private static boolean isSignatureFormatOk(OggVorbisProbe p) {
        return p.vorbis()
            && p.structurallyComplete()
            && p.channels() == 1
            && p.sampleRate() == SIGNATURE_SAMPLE_RATE
            && p.durationSeconds() <= SIGNATURE_MAX_SECONDS + SIGNATURE_DURATION_EPS;
    }

    @Test
    void everySignatureOggIsMonoVorbis44kUnderThreeSeconds() throws IOException {
        JsonObject root = soundsJson();
        for (String event : root.keySet()) {
            for (JsonElement el : root.getAsJsonObject(event).getAsJsonArray("sounds")) {
                String name = soundName(el);
                Path ogg = SOUNDS_DIR.resolve(name.substring("bong:".length()) + ".ogg");
                OggVorbisProbe p = probeOggVorbis(Files.readAllBytes(ogg));
                assertTrue(p.vorbis(),
                    "事件 " + event + " 的资产不是 Ogg Vorbis 容器（可能是伪 OGG / 换错格式）: " + ogg);
                assertTrue(p.structurallyComplete(),
                    "事件 " + event + " 的 ogg 容器不完整（framing/EOS/granule 缺失 → 截断或无音频）: " + ogg);
                assertEquals(1, p.channels(),
                    "事件 " + event + " 的 ogg 必须 mono（plan §P4 音源规格），实际声道=" + p.channels() + " @ " + ogg);
                assertEquals(SIGNATURE_SAMPLE_RATE, p.sampleRate(),
                    "事件 " + event + " 的 ogg 必须 44.1kHz，实际=" + p.sampleRate() + "Hz @ " + ogg);
                assertTrue(p.durationSeconds() <= SIGNATURE_MAX_SECONDS + SIGNATURE_DURATION_EPS,
                    "事件 " + event + " 的 ogg 必须 ≤3s（短样本），实际=" + p.durationSeconds() + "s @ " + ogg);
            }
        }
    }

    // ── 合成 Ogg fixture（证明格式校验的错误分支真会判红；每个 fixture 只破坏一项以精准定位）──

    /** 构造最小单页 Ogg：capture "OggS" + header_type（EOS）+ granule + 单 segment 承载 payload（payload<255）。 */
    private static byte[] buildOggPage(byte[] payload, long granule, boolean eos) {
        byte[] page = new byte[28 + payload.length];
        page[0] = 'O';
        page[1] = 'g';
        page[2] = 'g';
        page[3] = 'S';
        page[5] = (byte) (eos ? 0x04 : 0x00); // header_type：bit2 = EOS
        for (int i = 0; i < 8; i++) {
            page[6 + i] = (byte) ((granule >> (8 * i)) & 0xFF);
        }
        page[26] = 1; // page_segments
        page[27] = (byte) payload.length; // segment_table[0]
        System.arraycopy(payload, 0, page, 28, payload.length);
        return page;
    }

    /** 完整 30 字节 Vorbis identification header：0x01 "vorbis" version(4)=0 channels(1) sample_rate(4) … framing=1。 */
    private static byte[] fullVorbisIdHeader(int channels, int sampleRate) {
        byte[] h = new byte[VORBIS_ID_HEADER_LEN];
        h[0] = 0x01;
        h[1] = 'v';
        h[2] = 'o';
        h[3] = 'r';
        h[4] = 'b';
        h[5] = 'i';
        h[6] = 's';
        h[11] = (byte) channels;
        h[12] = (byte) (sampleRate & 0xFF);
        h[13] = (byte) ((sampleRate >> 8) & 0xFF);
        h[14] = (byte) ((sampleRate >> 16) & 0xFF);
        h[15] = (byte) ((sampleRate >> 24) & 0xFF);
        h[29] = 0x01; // framing flag
        return h;
    }

    /** 除被测属性外一切合法的签名 ogg（完整 header + framing + EOS + granule>0）。 */
    private static byte[] validSignatureOgg(int channels, int sampleRate, long granule, boolean eos) {
        return buildOggPage(fullVorbisIdHeader(channels, sampleRate), granule, eos);
    }

    @Test
    void audioFormatProbeRoundTripsFullyValidSyntheticOgg() {
        OggVorbisProbe ok = probeOggVorbis(validSignatureOgg(1, 44100, 44100, true));
        assertTrue(ok.vorbis(), "合成的合法 mono/44.1k header 应判定为 Vorbis");
        assertTrue(ok.structurallyComplete(), "完整 header + framing + EOS + granule>0 应判定结构完整");
        assertEquals(1, ok.channels());
        assertEquals(44100, ok.sampleRate());
        assertEquals(1.0, ok.durationSeconds(), 1e-9, "granule 44100 / 44100Hz 应算出 1.0s");
        assertTrue(isSignatureFormatOk(ok), "合法 mono/44.1k/1s/结构完整 应通过签名格式契约");
    }

    @Test
    void audioFormatContractRejectsOutOfSpecFixtures() {
        // 每个 fixture 仅破坏一项、其余全合法，确保「判红」归因到被测属性而非旁路缺陷。

        // stereo
        OggVorbisProbe stereo = probeOggVorbis(validSignatureOgg(2, 44100, 44100, true));
        assertEquals(2, stereo.channels(), "探针应正确读出 stereo 声道数");
        assertFalse(isSignatureFormatOk(stereo), "stereo 必须被格式契约判红");

        // 48kHz
        OggVorbisProbe wrongRate = probeOggVorbis(validSignatureOgg(1, 48000, 48000, true));
        assertEquals(48000, wrongRate.sampleRate(), "探针应读出 48kHz");
        assertFalse(isSignatureFormatOk(wrongRate), "48kHz 必须被判红");

        // 超长：granule = 4s 采样
        OggVorbisProbe tooLong = probeOggVorbis(validSignatureOgg(1, 44100, 4L * 44100, true));
        assertTrue(tooLong.durationSeconds() > SIGNATURE_MAX_SECONDS, "探针应算出时长 >3s");
        assertFalse(isSignatureFormatOk(tooLong), ">3s 必须被判红");

        // 缺 EOS：结构不完整（截断/未收尾）
        OggVorbisProbe noEos = probeOggVorbis(validSignatureOgg(1, 44100, 44100, false));
        assertFalse(noEos.structurallyComplete(), "缺 EOS 应判定结构不完整");
        assertFalse(isSignatureFormatOk(noEos), "缺 EOS（可能截断）必须被判红");

        // granule=0：无 PCM 采样（空音频）
        OggVorbisProbe zeroGranule = probeOggVorbis(validSignatureOgg(1, 44100, 0, true));
        assertFalse(zeroGranule.structurallyComplete(), "granule=0 应判定结构不完整");
        assertFalse(isSignatureFormatOk(zeroGranule), "granule=0（无音频）必须被判红");

        // 截断 identification header（16 字节，无 framing flag）：有效魔数前缀但 header 不完整
        OggVorbisProbe truncated = probeOggVorbis(buildOggPage(truncatedVorbisIdHeader(), 44100, true));
        assertTrue(truncated.vorbis(), "有 OggS + vorbis 前缀，探针仍识别为 Vorbis codec");
        assertFalse(truncated.structurallyComplete(), "截断 header（无 framing）应判定结构不完整");
        assertFalse(isSignatureFormatOk(truncated), "截断伪 Vorbis 必须被判红（本轮 review 核心缺口）");

        // 伪 OGG / 损坏：无 OggS capture
        OggVorbisProbe notOgg = probeOggVorbis("NOT_AN_OGG_FILE".getBytes(StandardCharsets.UTF_8));
        assertFalse(notOgg.vorbis(), "非 Ogg 数据应判定 vorbis=false");
        assertFalse(isSignatureFormatOk(notOgg), "非 Vorbis 必须被判红");

        // 有 OggS 但 identification header 不是 Vorbis
        OggVorbisProbe notVorbis = probeOggVorbis(buildOggPage("OpusHead".getBytes(StandardCharsets.UTF_8), 0, true));
        assertFalse(notVorbis.vorbis(), "Ogg 容器但非 Vorbis identification header 应判定 vorbis=false");
        assertFalse(isSignatureFormatOk(notVorbis), "非 Vorbis codec 必须被判红");
    }

    /** 截断的 Vorbis id header：只有 16 字节（到 sample_rate 为止），无 framing flag → 结构不完整。 */
    private static byte[] truncatedVorbisIdHeader() {
        byte[] h = new byte[16];
        h[0] = 0x01;
        h[1] = 'v';
        h[2] = 'o';
        h[3] = 'r';
        h[4] = 'b';
        h[5] = 'i';
        h[6] = 's';
        h[11] = 1;
        h[12] = (byte) (44100 & 0xFF);
        h[13] = (byte) ((44100 >> 8) & 0xFF);
        h[14] = (byte) ((44100 >> 16) & 0xFF);
        h[15] = (byte) ((44100 >> 24) & 0xFF);
        return h;
    }
}
