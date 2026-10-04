package com.bong.client.network;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * RF-28 R6 P4 的 C2S 客户端契约 pin。
 *
 * <p>生成的 TypeBox registry 是跨端 live type 集合；Java 协议类必须为每个
 * live type 保留一个显式 envelope 编码入口。测试只比较 wire type 集合，不绑定
 * 编码方法数量或实现细节，从而允许同一入口承载多个业务重载。</p>
 */
class ClientRequestContractPinTest {
    private static final int LIVE_C2S_TYPE_COUNT = 102;
    private static final Path GENERATED_SCHEMA = Path.of(
        "..", "agent", "packages", "schema", "generated", "client-request-v1.json"
    );
    private static final Path PROTOCOL_SOURCE = Path.of(
        "src", "main", "java", "com", "bong", "client", "network", "ClientRequestProtocol.java"
    );
    private static final Pattern ENVELOPE_LITERAL = Pattern.compile(
        "envelope\\(\\\"([a-z0-9_]+)\\\"\\)"
    );

    @Test
    void generatedLiveTypesHaveExplicitJavaEncoders() throws IOException {
        Set<String> generatedTypes = generatedTypes();
        Set<String> javaEncoderTypes = javaEncoderTypes();

        assertEquals(
            LIVE_C2S_TYPE_COUNT,
            generatedTypes.size(),
            "C2S live registry count changed; update the pin only after reviewing both registries"
        );
        assertEquals(
            generatedTypes,
            javaEncoderTypes,
            "every generated live C2S type must have exactly a matching Java envelope literal"
        );
    }

    private static Set<String> generatedTypes() throws IOException {
        JsonObject root = JsonParser.parseString(
            Files.readString(GENERATED_SCHEMA, StandardCharsets.UTF_8)
        ).getAsJsonObject();
        JsonArray variants = root.getAsJsonArray("anyOf");
        Set<String> types = new TreeSet<>();
        variants.forEach(variant -> {
            JsonObject properties = variant.getAsJsonObject().getAsJsonObject("properties");
            types.add(properties.getAsJsonObject("type").get("const").getAsString());
        });
        return types;
    }

    private static Set<String> javaEncoderTypes() throws IOException {
        String source = Files.readString(PROTOCOL_SOURCE, StandardCharsets.UTF_8);
        Matcher matcher = ENVELOPE_LITERAL.matcher(source);
        Set<String> types = new TreeSet<>();
        while (matcher.find()) {
            types.add(matcher.group(1));
        }
        return types;
    }
}
