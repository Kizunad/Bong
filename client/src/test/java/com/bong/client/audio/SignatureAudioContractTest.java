package com.bong.client.audio;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonReader;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.StringReader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;

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
 *   <li>第 8 招 heaven_gate 的 client recipe 必须**真实引用**其 {@code bong:} 事件（{@link #heavenGateRecipesConsumeTheirBongEvent}）——
 *       server 侧只 pin 了另外 7 招，这里补齐 8/8 逐项消费锁；把 recipe 主层改回 {@code minecraft:} 立即判红；</li>
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
     * 第 8 招 heaven_gate 的 client recipe → 必须消费的 {@code bong:} 事件逐项 pin。
     * charge 尾程（蓄满顶点，plan「charge 尾程 + release」范围）与 release 都必须真实引用签名事件；
     * server 侧 {@code each_server_signature_recipe_uses_its_bong_event} 已 pin 另外 7 招。
     */
    private static final Map<String, String> HEAVEN_GATE_RECIPE_PINS = Map.of(
        "heaven_gate_release.json", "bong:skill.sword_path.heaven_gate",
        "heaven_gate_charge_2s.json", "bong:skill.sword_path.heaven_gate");

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
     * 第 8 招逐项消费锁：heaven_gate 的 client recipe（charge 尾程 + release）必须**真实引用**其
     * signature 事件。把主层改回 {@code minecraft:} 会立即判红——补齐 plan 要求的 8/8 逐项 pin
     * （server 侧只 pin 了另外 7 招）。
     */
    @Test
    void heavenGateRecipesConsumeTheirBongEvent() throws IOException {
        for (Map.Entry<String, String> pin : HEAVEN_GATE_RECIPE_PINS.entrySet()) {
            Path recipeFile = CLIENT_RECIPE_DIR.resolve(pin.getKey());
            String expectedEvent = pin.getValue();
            assertTrue(Files.isRegularFile(recipeFile),
                "heaven_gate 逐项 pin 的 recipe 缺失: " + recipeFile.toAbsolutePath());
            JsonObject recipe = JsonParser.parseString(Files.readString(recipeFile)).getAsJsonObject();
            assertTrue(recipe.has("layers"), recipeFile.getFileName() + " 必须含 layers");
            boolean consumes = false;
            for (JsonElement layerEl : recipe.getAsJsonArray("layers")) {
                if (expectedEvent.equals(layerEl.getAsJsonObject().get("sound").getAsString())) {
                    consumes = true;
                    break;
                }
            }
            assertTrue(consumes,
                "recipe " + pin.getKey() + " 必须有一层真实引用签名事件 " + expectedEvent
                    + "（防资产只注册不消费——若此处改回 minecraft: 则第 8 招签名音永不触发）");
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
}
