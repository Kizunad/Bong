/**
 * Test-only cross-process adapter for the production UiResponseConsumer.
 *
 * stdin: exactly one AgentUiResponsePayloadV1 JSON document
 * stdout: exactly the raw NarrationV1 JSON published to AGENT_NARRATE
 *
 * This adapter deliberately exposes no production channel or protocol. Rust integration
 * tests use it to execute the TypeScript consumer rather than substituting a narration fixture.
 */

import { CHANNELS } from "@bong/schema";
import { UiResponseConsumer } from "../src/ui/uiResponseConsumer.js";

const inputChunks: Buffer[] = [];
for await (const chunk of process.stdin) {
  inputChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
}
const input = Buffer.concat(inputChunks).toString("utf8");
const publications: Array<{ channel: string; message: string }> = [];
const warnings: unknown[][] = [];

const consumer = new UiResponseConsumer({
  sub: {
    subscribe: async () => undefined,
    on: () => undefined,
    off: () => undefined,
    unsubscribe: async () => undefined,
    disconnect: () => undefined,
  },
  pub: {
    publish: async (channel, message) => {
      publications.push({ channel, message });
      return 1;
    },
    disconnect: () => undefined,
  },
  logger: {
    info: () => undefined,
    warn: (...args) => warnings.push(args),
  },
});

await consumer.handlePayload(input);

if (warnings.length > 0) {
  throw new Error(`consumer emitted unexpected warnings: ${JSON.stringify(warnings)}`);
}
if (consumer.stats.rejectedContract !== 0) {
  throw new Error("consumer rejected the producer response contract");
}
if (consumer.stats.realmGateRejected !== 1 || consumer.stats.narrationPublished !== 1) {
  throw new Error(`unexpected consumer stats: ${JSON.stringify(consumer.stats)}`);
}
if (publications.length !== 1 || publications[0]?.channel !== CHANNELS.AGENT_NARRATE) {
  throw new Error(
    `expected one ${CHANNELS.AGENT_NARRATE} publication: ${JSON.stringify(publications)}`,
  );
}

process.stdout.write(publications[0].message);
