import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const workflowPath = new URL('../workflows/review-next.yml', import.meta.url);
const consumerTestsWorkflowPath = new URL('../workflows/review-consumer-tests.yml', import.meta.url);
const canaryWorkflowPath = new URL('../workflows/review-provider-canary.yml', import.meta.url);
const canaryContractPath = '.github/workflows/provider-canary.yml';
const policyPath = new URL('../review-policy/bong.v2.json', import.meta.url);
const centralSha = '2986b95388b3ac3f8bf28b8ebdd60a19d2e4364c';
const providerCanarySha = '9dcee849e3a0b45bd9a8fe663b48ae3fb1d82784';
// Hash of Kizunad/review/.github/workflows/review.yml; re-verify it on every pin bump.
// It changes only when that workflow changes, which a central pin bump usually does not.
const centralWorkflowSha256 = 'b07ba74a0eff87a520a5bd81da4cc7eb0ead97025680af5f5242db9c8e7153e8';

const expectedCanaryInterface = `  workflow_call:
    inputs:
      review_base_url:
        description: Claude-compatible provider base URL
        required: true
        type: string
      worker_timeout_ms:
        description: Per-model canary timeout
        required: false
        default: 60000
        type: number
    secrets:
      review_api_key:
        description: Caller-owned provider credential
        required: true`;

const expectedCentralInterface = `  workflow_call:
    inputs:
      pr_number:
        description: Pull request number in the calling repository
        required: true
        type: number
      policy_path:
        description: Trusted base-revision review policy path
        required: true
        type: string
      review_base_url:
        description: Claude-compatible provider base URL
        required: true
        type: string
      shadow:
        description: Publish a non-gating shadow review
        required: false
        default: false
        type: boolean
      max_diff_chars:
        description: Maximum immutable diff characters per Terra finder
        required: false
        default: 40000
        type: number
      max_shard_chars:
        description: Maximum Luna shard characters
        required: false
        default: 12000
        type: number
      worker_timeout_ms:
        description: Per-Claude-process timeout
        required: false
        default: 120000
        type: number
      circuit_manual_retry:
        description: Allow a trusted workflow_dispatch retry to bypass an open infrastructure circuit
        required: false
        default: false
        type: boolean
    secrets:
      review_api_key:
        description: Caller-owned provider credential
        required: true`;

const expectedCentralJobs = Object.freeze({
  preflight: `    permissions:
      actions: read
      contents: read
      pull-requests: read
      issues: read`,
  review: `    permissions:
      contents: read
      pull-requests: read`,
  finalize: `    permissions:
      contents: read
      pull-requests: write
      issues: write`,
});

const expectedPolicyLevels = Object.freeze({
  'plan-intent': 'major',
  'production-wiring': 'major',
  'three-layer-contract': 'major',
  'schema-source-of-truth': 'major',
  'wire-format-bridge': 'major',
  'qi-conservation': 'blocker',
  'qi-constants': 'major',
  'qi-era-decay': 'blocker',
  'worldview-realms': 'major',
  'worldview-economy': 'major',
  'worldview-naming': 'major',
  'zone-identity': 'major',
  'layer-registry': 'major',
  'raster-contract': 'major',
  'bevy-ecs-boundary': 'major',
  'no-vanilla-entity-hack': 'major',
  'bot-client-tolerance': 'major',
  'skill-full-stack-av': 'major',
  'conditional-hud': 'major',
  'player-animation-contract': 'major',
  'resourcepack-binding': 'major',
  'inventory-category': 'major',
  'saturated-tests': 'major',
  'test-failure-honesty': 'major',
  'minimal-maintainable-change': 'major',
  'p0-doc-scope': 'major',
  'quality-improvements': 'suggestion',
});

const expectedCallerJobs = `jobs:
  central-review-shadow:
    if: >-
      github.event_name == 'workflow_dispatch' ||
      (github.event.issue.pull_request != null &&
       github.event.comment.body == '/review-next' &&
       contains(fromJSON('["OWNER","MEMBER","COLLABORATOR"]'), github.event.comment.author_association))
    permissions:
      actions: read
      contents: read
      pull-requests: write
      issues: write
    uses: Kizunad/review/.github/workflows/review.yml@2986b95388b3ac3f8bf28b8ebdd60a19d2e4364c
    with:
      pr_number: \${{ fromJSON(github.event.issue.number || inputs.pr_number) }}
      policy_path: .github/review-policy/bong.v2.json
      review_base_url: \${{ vars.REVIEW_CLAUDE_BASE_URL || 'https://api.claudeopus.world' }}
      shadow: false
      max_diff_chars: 40000
      max_shard_chars: 12000
      # 300s, not the 120s default: the relay's upstream routinely takes 60-135s
      # per request under load, so 120s left no room for a single retry and the
      # summary stage died to "timeout after 120000ms" while requests were live.
      worker_timeout_ms: 300000
      circuit_manual_retry: \${{ github.event_name == 'workflow_dispatch' }}
    secrets:
      review_api_key: \${{ secrets.REVIEW_CLAUDE_API_KEY }}`;

async function workflow() {
  return readFile(workflowPath, 'utf8');
}

async function consumerTestsWorkflow() {
  return readFile(consumerTestsWorkflowPath, 'utf8');
}

async function canaryWorkflow() {
  return readFile(canaryWorkflowPath, 'utf8');
}

async function policy() {
  return JSON.parse(await readFile(policyPath, 'utf8'));
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function exactSection(yaml, startMarker, endMarker, name) {
  const startMatches = yaml.match(new RegExp(`^${startMarker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`, 'gm')) ?? [];
  assert.equal(startMatches.length, 1, `${name} must appear exactly once`);
  const start = yaml.indexOf(startMarker);
  const end = endMarker === null ? yaml.length : yaml.indexOf(endMarker, start + startMarker.length);
  assert.ok(end >= 0, `${name} end marker is missing`);
  return yaml.slice(start, end).trimEnd();
}

function assertExactWorkflowCall(yaml, expected, name) {
  const actual = exactSection(yaml, '  workflow_call:', '\npermissions:', `${name} workflow_call`);
  assert.equal(actual, expected, `${name} workflow_call interface drifted`);
}

function assertExactCentralInterface(yaml) {
  assertExactWorkflowCall(yaml, expectedCentralInterface, 'central');
}

function topLevelJobNames(yaml) {
  const jobsStart = yaml.indexOf('\njobs:\n');
  assert.ok(jobsStart >= 0, 'central jobs section is missing');
  const jobsText = yaml.slice(jobsStart + 1);
  return [...jobsText.matchAll(/^  ([a-z][a-z0-9_-]*):$/gm)].map((match) => match[1]);
}

function exactJobSection(yaml, jobName, nextJobName) {
  const startMarker = `  ${jobName}:`;
  const endMarker = nextJobName === null ? null : `\n  ${nextJobName}:`;
  return exactSection(yaml, startMarker, endMarker, `central job ${jobName}`);
}

function assertExactCentralPermissions(yaml) {
  const jobNames = topLevelJobNames(yaml);
  assert.deepEqual(jobNames, Object.keys(expectedCentralJobs), 'central job set or order drifted');
  for (const [index, jobName] of jobNames.entries()) {
    const nextJobName = jobNames[index + 1] ?? null;
    const section = exactJobSection(yaml, jobName, nextJobName);
    const declarations = section.match(/^    permissions:(?: .*|\n(?:^      .*\n?)*)/gm) ?? [];
    assert.equal(declarations.length, 1, `${jobName} must declare permissions exactly once`);
    assert.equal(
      declarations[0].trimEnd(),
      expectedCentralJobs[jobName],
      `${jobName} permission requirements drifted`,
    );
  }

  const allDeclarations = yaml.match(/^permissions:.*$|^    permissions:.*$/gm) ?? [];
  assert.deepEqual(
    allDeclarations,
    ['permissions: {}', ...jobNames.map(() => '    permissions:')],
    'central workflow contains an unexpected permissions declaration',
  );
}

function assertExactCallerJobs(yaml) {
  const actual = exactSection(yaml, 'jobs:', null, 'caller jobs');
  assert.equal(actual, expectedCallerJobs, 'caller reusable job contract drifted');
}

function centralFixture(interfaceBlock = expectedCentralInterface) {
  return `name: fixture

on:
${interfaceBlock}

permissions: {}
`;
}

function callerFixture(jobsBlock = expectedCallerJobs) {
  return `name: fixture

on:
  workflow_dispatch:

permissions: {}

${jobsBlock}
`;
}

test('shadow caller pins the central workflow and preserves the trusted trigger gate', async () => {
  const yaml = await workflow();
  assert.match(yaml, /^  issue_comment:\n    types: \[created\]$/m);
  assert.match(yaml, /^  workflow_dispatch:/m);
  assert.match(yaml, /^permissions: \{\}$/m);
  assert.match(yaml, /github\.event\.comment\.body == '\/review-next'/);
  assert.doesNotMatch(yaml, /startsWith\([^\n]*\/review-next/);
  assert.match(yaml, /\["OWNER","MEMBER","COLLABORATOR"\]/);
  assert.match(
    yaml,
    /uses: Kizunad\/review\/\.github\/workflows\/review\.yml@2986b95388b3ac3f8bf28b8ebdd60a19d2e4364c/,
  );
  assert.doesNotMatch(yaml, /Kizunad\/review\/[^\n]*@(main|master|v?\d|[0-9a-f]{1,39})\b/);
  assert.match(yaml, /pr_number: \$\{\{ fromJSON\(github\.event\.issue\.number \|\| inputs\.pr_number\) \}\}/);
  assert.match(yaml, /shadow: false/);
  assert.match(yaml, /worker_timeout_ms: 300000/);
  assert.match(yaml, /circuit_manual_retry: \$\{\{ github\.event_name == 'workflow_dispatch' \}\}/);
  assert.match(yaml, /policy_path: \.github\/review-policy\/bong\.v2\.json/);
  assert.match(yaml, /review_base_url: \$\{\{ vars\.REVIEW_CLAUDE_BASE_URL \|\| 'https:\/\/api\.claudeopus\.world' \}\}/);
});

test('shadow caller job exactly matches the reviewed reusable contract', async () => {
  const yaml = await workflow();
  assertExactCallerJobs(yaml);
  assert.doesNotMatch(yaml, /secrets:\s*inherit/);
  assert.doesNotMatch(yaml, /REVIEW_CODEX_API_KEY|PI_CLIPROXY_KEY|HLOOL_API_KEY|OPENAI_API_KEY/);
  assert.equal((yaml.match(/uses: Kizunad\/review\//g) ?? []).length, 1);
});

test('consumer CI checks out and tests the exact central workflow contract', async () => {
  const yaml = await consumerTestsWorkflow();
  assert.match(yaml, /repository: Kizunad\/review/);
  assert.match(yaml, new RegExp(`ref: ${centralSha}`));
  assert.ok(
    yaml.includes(`[[ "$(git -C _central-contract rev-parse HEAD)" == '${centralSha}' ]]`),
    'consumer CI must verify the checked-out central OID',
  );
  assert.match(yaml, new RegExp(`ref: ${providerCanarySha}`));
  assert.match(yaml, new RegExp(`path: ${providerCanarySha}`));
  assert.ok(
    yaml.includes(`[[ "$(git -C ${providerCanarySha} rev-parse HEAD)" == '${providerCanarySha}' ]]`),
    'consumer CI must verify the checked-out provider canary OID',
  );
  assert.match(yaml, new RegExp(`PROVIDER_CANARY_CONTRACT_DIR=${providerCanarySha}`));
  assert.match(yaml, /bubblewrap_0\.9\.0-1ubuntu0\.1_amd64\.deb/);
  assert.match(yaml, /1b506492bd9c7fd0cdb4f02ac822f1d3e336b0aead5113c1239baf8db5db562a/);
  assert.match(yaml, /sha256sum --check --strict/);
  assert.match(yaml, /root:root 4755/);
  assert.match(yaml, /BWRAP_EXECUTABLE=%s/);
  assert.match(yaml, /(?:\n\s+cd _central-contract\n\s+npm test\n)/);
  assert.doesNotMatch(yaml, /repository: Kizunad\/review[\s\S]*?ref: (?:main|master|v?\d|[0-9a-f]{1,39})\b/);
});

test('checked-out central workflow matches the complete immutable publication contract', async (context) => {
  const centralRoot = process.env.CENTRAL_REVIEW_CONTRACT_DIR;
  if (!centralRoot) {
    context.skip('CENTRAL_REVIEW_CONTRACT_DIR is required for the cross-repository contract check');
    return;
  }
  const centralYaml = await readFile(path.join(centralRoot, '.github/workflows/review.yml'), 'utf8');
  assert.equal(sha256(centralYaml), centralWorkflowSha256, 'pinned central workflow bytes drifted');
  assertExactCentralInterface(centralYaml);
  assertExactCentralPermissions(centralYaml);
  assert.match(centralYaml, /^permissions: \{\}$/m);
});

test('central publication contract rejects every input and secret set drift', () => {
  assert.doesNotThrow(() => assertExactCentralInterface(centralFixture()));
  for (const [name, mutation] of [
    ['required input addition', expectedCentralInterface.replace(
      '      shadow:',
      '      added_required_input:\n        required: true\n        type: string\n      shadow:',
    )],
    ['optional input addition', expectedCentralInterface.replace(
      '      shadow:',
      '      added_optional_input:\n        required: false\n        default: 7\n        type: number\n      shadow:',
    )],
    ['input removal', expectedCentralInterface.replace(
      '      shadow:\n        description: Publish a non-gating shadow review\n        required: false\n        default: false\n        type: boolean\n',
      '',
    )],
    ['required secret addition', expectedCentralInterface.replace(
      '      review_api_key:',
      '      added_required_secret:\n        required: true\n      review_api_key:',
    )],
    ['optional secret addition', expectedCentralInterface.replace(
      '      review_api_key:',
      '      added_optional_secret:\n        required: false\n      review_api_key:',
    )],
    ['secret removal', expectedCentralInterface.replace(
      '    secrets:\n      review_api_key:\n        description: Caller-owned provider credential\n        required: true',
      '    secrets: {}',
    )],
  ]) {
    assert.throws(
      () => assertExactCentralInterface(centralFixture(mutation)),
      { message: /central workflow_call interface drifted/ },
      name,
    );
  }
});

test('caller publication contract rejects mapping, secret, and permission drift', () => {
  assert.doesNotThrow(() => assertExactCallerJobs(callerFixture()));
  for (const [name, mutation] of [
    ['missing input', expectedCallerJobs.replace('      policy_path: .github/review-policy/bong.v2.json\n', '')],
    ['unknown input', expectedCallerJobs.replace(
      '      policy_path: .github/review-policy/bong.v2.json',
      '      policy_path: .github/review-policy/bong.v2.json\n      unknown_input: value',
    )],
    ['missing secret', expectedCallerJobs.replace('      review_api_key: ${{ secrets.REVIEW_CLAUDE_API_KEY }}', '')],
    ['unknown secret', expectedCallerJobs.replace(
      '      review_api_key: ${{ secrets.REVIEW_CLAUDE_API_KEY }}',
      '      review_api_key: ${{ secrets.REVIEW_CLAUDE_API_KEY }}\n      unknown_secret: value',
    )],
    ['insufficient permission', expectedCallerJobs.replace('      pull-requests: write', '      pull-requests: read')],
    ['excessive permission', expectedCallerJobs.replace('      issues: write', '      issues: write\n      checks: write')],
  ]) {
    assert.throws(
      () => assertExactCallerJobs(callerFixture(mutation)),
      { message: /caller reusable job contract drifted/ },
      name,
    );
  }
});

test('central permission contract rejects unbound and non-block permission forms', () => {
  const permissionFixture = `permissions: {}

jobs:
  preflight:
${expectedCentralJobs.preflight}
  review:
${expectedCentralJobs.review}
  finalize:
${expectedCentralJobs.finalize}
`;
  assert.doesNotThrow(() => assertExactCentralPermissions(permissionFixture));
  for (const [name, mutation] of [
    ['missing permission', permissionFixture.replace('      actions: read\n', '')],
    ['additional permission', permissionFixture.replace(
      '      issues: write',
      '      issues: write\n      checks: write',
    )],
    ['scalar write-all', permissionFixture.replace(
      `  finalize:\n${expectedCentralJobs.finalize}`,
      '  finalize:\n    permissions: write-all',
    )],
    ['inline mapping', permissionFixture.replace(
      `  review:\n${expectedCentralJobs.review}`,
      '  review:\n    permissions: {contents: read, pull-requests: write}',
    )],
    ['empty mapping', permissionFixture.replace(
      `  review:\n${expectedCentralJobs.review}`,
      '  review:\n    permissions: {}',
    )],
    ['additional privileged job', `${permissionFixture}  publish:\n    permissions: write-all\n`],
    ['job rename', permissionFixture.replace('  review:', '  inspect:')],
    ['job permission swap', permissionFixture
      .replace(expectedCentralJobs.preflight, '__PREFLIGHT__')
      .replace(expectedCentralJobs.finalize, expectedCentralJobs.preflight)
      .replace('__PREFLIGHT__', expectedCentralJobs.finalize)],
  ]) {
    assert.throws(
      () => assertExactCentralPermissions(mutation),
      undefined,
      name,
    );
  }
});

test('checked-out provider canary release matches its caller interface', async (context) => {
  const canaryRoot = process.env.PROVIDER_CANARY_CONTRACT_DIR;
  if (!canaryRoot) {
    context.skip('PROVIDER_CANARY_CONTRACT_DIR is required for the provider canary contract check');
    return;
  }
  assert.equal(
    path.basename(canaryRoot),
    providerCanarySha,
    'provider canary contract directory must be named for the immutable release SHA',
  );
  const centralYaml = await readFile(path.join(canaryRoot, canaryContractPath), 'utf8');
  assertExactWorkflowCall(centralYaml, expectedCanaryInterface, 'provider canary');
  assert.match(centralYaml, /^permissions: \{\}$/m);
  assert.match(centralYaml, /^    permissions:\n      # Used only to resolve this reusable workflow's immutable referenced_workflows SHA\.\n      actions: read$/m);
  assert.doesNotMatch(centralYaml, /contents:|pull-requests:|issues:/);
});

test('provider canary publication contract rejects every input and secret set drift', () => {
  const fixture = `name: fixture\n\non:\n${expectedCanaryInterface}\n\npermissions: {}\n`;
  assert.doesNotThrow(() => assertExactWorkflowCall(fixture, expectedCanaryInterface, 'provider canary'));
  for (const [name, mutation] of [
    ['required input addition', expectedCanaryInterface.replace(
      '      worker_timeout_ms:',
      '      added_required_input:\n        required: true\n        type: string\n      worker_timeout_ms:',
    )],
    ['input removal', expectedCanaryInterface.replace(
      '      worker_timeout_ms:\n        description: Per-model canary timeout\n        required: false\n        default: 60000\n        type: number\n',
      '',
    )],
    ['input type drift', expectedCanaryInterface.replace('        type: number', '        type: string')],
    ['input default drift', expectedCanaryInterface.replace('        default: 60000', '        default: 120000')],
    ['required secret addition', expectedCanaryInterface.replace(
      '      review_api_key:',
      '      added_required_secret:\n        required: true\n      review_api_key:',
    )],
    ['secret removal', expectedCanaryInterface.replace(
      '    secrets:\n      review_api_key:\n        description: Caller-owned provider credential\n        required: true',
      '    secrets: {}',
    )],
  ]) {
    const mutated = fixture.replace(expectedCanaryInterface, mutation);
    assert.throws(
      () => assertExactWorkflowCall(mutated, expectedCanaryInterface, 'provider canary'),
      { message: /provider canary workflow_call interface drifted/ },
      name,
    );
  }
});

test('provider canary remains dispatch-only, minimally permissioned, and secret-isolated', async () => {
  const yaml = await canaryWorkflow();
  assert.match(yaml, /^  workflow_dispatch:$/m);
  assert.doesNotMatch(yaml, /^  (?:pull_request|pull_request_target|issue_comment|push|schedule):/m);
  assert.match(yaml, /^permissions: \{\}$/m);
  assert.match(yaml, /permissions:\n      actions: read/);
  assert.doesNotMatch(yaml, /contents:|pull-requests:|issues:/);
  assert.match(
    yaml,
    new RegExp(`^    uses: Kizunad/review/\\.github/workflows/provider-canary\\.yml@${providerCanarySha}$`, 'm'),
  );
  assert.equal(
    (yaml.match(/uses: Kizunad\/review\/\.github\/workflows\/provider-canary\.yml@[0-9a-f]{40}/g) ?? []).length,
    1,
    'provider canary must use exactly one immutable central release',
  );
  assert.doesNotMatch(yaml, /provider-canary\.yml@(main|master|v?\d|[0-9a-f]{1,39})\b/);
  assert.match(yaml, /review_base_url: \$\{\{ vars\.REVIEW_CLAUDE_BASE_URL \|\| 'https:\/\/api\.claudeopus\.world' \}\}/);
  assert.match(yaml, /worker_timeout_ms: 60000/);
  assert.match(yaml, /review_api_key: \$\{\{ secrets\.REVIEW_CLAUDE_API_KEY \}\}/);
  assert.doesNotMatch(yaml, /secrets:\s*inherit|CLAUDE_CODE_OAUTH_TOKEN|PI_CLIPROXY_KEY|OPENAI_API_KEY/);
});

test('Bong policy is bounded declarative data with canonical project rules', async () => {
  const value = await policy();
  assert.deepEqual(Object.keys(value).sort(), [
    'minorFindingsRequestChanges', 'project', 'rules', 'version',
  ]);
  assert.equal(value.version, 'project-review-policy.v2');
  assert.equal(value.project, 'Kizunad/Bong');
  assert.equal(value.minorFindingsRequestChanges, false);
  assert.ok(value.rules.length >= 20 && value.rules.length <= 256);

  const ids = new Set();
  for (const rule of value.rules) {
    assert.deepEqual(Object.keys(rule).sort(), ['id', 'level', 'text']);
    assert.match(rule.id, /^[a-z][a-z0-9-]{0,79}$/);
    assert.ok(!ids.has(rule.id), `duplicate policy rule: ${rule.id}`);
    ids.add(rule.id);
    assert.ok(['blocker', 'major', 'minor', 'suggestion'].includes(rule.level));
    assert.ok(rule.text.length >= 1 && rule.text.length <= 4000);
  }

  for (const required of [
    'plan-intent',
    'production-wiring',
    'three-layer-contract',
    'schema-source-of-truth',
    'qi-conservation',
    'worldview-realms',
    'worldview-economy',
    'layer-registry',
    'skill-full-stack-av',
    'saturated-tests',
    'quality-improvements',
  ]) {
    assert.ok(ids.has(required), `missing Bong policy rule: ${required}`);
  }

  const byId = Object.fromEntries(
    value.rules.map((rule) => [rule.id, rule]),
  );
  assert.deepEqual(
    Object.fromEntries(value.rules.map((rule) => [rule.id, rule.level])),
    expectedPolicyLevels,
    'Bong policy rule set and levels must remain explicit',
  );
  assert.equal(byId['qi-conservation'].level, 'blocker');
  assert.match(
    byId['qi-conservation'].text,
    /conservation.*merge blockers/i,
  );
  assert.equal(byId['saturated-tests'].level, 'major');
  for (const requiredContract of [
    /happy paths/i,
    /boundaries.*empty.*maximum.*off-by-one/i,
    /invalid inputs.*every error branch/i,
    /permission and state preconditions/i,
    /every enum variant/i,
    /every state transition.*self or no-op transitions/i,
    /Schema, enum, and state-machine changes require dedicated positive and negative pin tests/i,
    /producer-to-wire-to-consumer.*integration test/i,
    /concrete incorrect implementation/,
    /falsely claim nonexistent coverage/,
    /Finer assertions.*already fully protected.*suggestions/i,
  ]) {
    assert.match(
      byId['saturated-tests'].text,
      requiredContract,
      `saturated-tests must preserve ${requiredContract}`,
    );
  }
  assert.equal(byId['minimal-maintainable-change'].level, 'major');
  for (const requiredRisk of [
    /Compatibility layers without an explicit migration need/i,
    /speculative abstractions without a present requirement/i,
    /inconsistent duplicate sources of truth/i,
    /broad refactors unrelated to the requested outcome/i,
    /major defects.*before a production failure is observed/i,
    /unreachable production wiring/i,
    /non-atomic state/i,
    /unbounded resource paths/i,
  ]) {
    assert.match(
      byId['minimal-maintainable-change'].text,
      requiredRisk,
      `minimal-maintainable-change must preserve ${requiredRisk}`,
    );
  }
  assert.equal(byId['p0-doc-scope'].level, 'major');
  assert.equal(
    byId['p0-doc-scope'].text,
    "For pull requests whose changes are limited to docs/ plan documents (plan-* and plans-skeleton design-closure PRs), limit review scope to internal consistency, factual accuracy of code citations, alignment with plan-refactor-master-v1 scope and cross-track dependencies, and satisfaction of the acceptance criteria stated in the PR description or the document itself. Depth expansions beyond those criteria - new scenarios, extra edge-case enumerations, additional contract surfaces - are suggestions for the open-questions section, not major or blocker findings; an explicit open-questions entry with rationale is a valid resolution for a deferred design decision.",
  );
  assert.equal(byId['quality-improvements'].level, 'suggestion');
  assert.match(
    byId['quality-improvements'].text,
    /merely restate obvious code.*clearer naming.*optional helper extraction.*finer assertions/s,
  );
  assert.match(
    byId['quality-improvements'].text,
    /bounded improvements do not gate the review/,
  );
  assert.match(
    byId['quality-improvements'].text,
    /does not downgrade.*speculative abstractions.*compatibility layers.*duplicate sources of truth.*unrelated scope growth.*missing required contract tests/s,
  );

  const serialized = JSON.stringify(value);
  assert.match(serialized, /醒灵.*引气.*凝脉.*固元.*通灵.*化虚/);
  assert.match(serialized, /骨币/);
  assert.match(serialized, /SPIRIT_QI_TOTAL/);
  assert.match(serialized, /LAYER_REGISTRY/);
  assert.doesNotMatch(serialized, /"severity"/);
  assert.doesNotMatch(serialized, /(?:^|\W)(?:command|shell|runner|action ref|MCP server)(?:\W|$)/i);
});
