import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const workflowPath = new URL('../workflows/review-next.yml', import.meta.url);
const consumerTestsWorkflowPath = new URL('../workflows/review-consumer-tests.yml', import.meta.url);
const canaryWorkflowPath = new URL('../workflows/review-provider-canary.yml', import.meta.url);
const policyPath = new URL('../review-policy/bong.v1.json', import.meta.url);
const centralSha = '417e55e55737b8fe42803b97f85b59fce8bbfb2a';

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

const expectedCentralInputs = Object.freeze({
  pr_number: { type: 'number', required: true },
  policy_path: { type: 'string', required: true },
  review_base_url: { type: 'string', required: true },
  shadow: { type: 'boolean', required: false, default: false },
  max_diff_chars: { type: 'number', required: false, default: 40000 },
  max_shard_chars: { type: 'number', required: false, default: 12000 },
  worker_timeout_ms: { type: 'number', required: false, default: 120000 },
  circuit_manual_retry: { type: 'boolean', required: false, default: false },
});

function parseYamlScalar(value) {
  const scalar = value.trim();
  if (scalar === 'true') return true;
  if (scalar === 'false') return false;
  if (/^-?(?:0|[1-9][0-9]*)$/.test(scalar)) return Number(scalar);
  if (
    scalar.length >= 2 &&
    ((scalar.startsWith("'") && scalar.endsWith("'")) ||
      (scalar.startsWith('"') && scalar.endsWith('"')))
  ) {
    return scalar.slice(1, -1);
  }
  return scalar;
}

function parseYamlMapping(lines, startIndex, parentIndent) {
  const value = {};
  let index = startIndex;
  while (index < lines.length) {
    const raw = lines[index];
    const trimmed = raw.trim();
    if (trimmed === '' || trimmed.startsWith('#')) {
      index += 1;
      continue;
    }

    const indent = raw.length - raw.trimStart().length;
    if (indent <= parentIndent) break;
    if (indent !== parentIndent + 2) {
      index += 1;
      continue;
    }

    const match = trimmed.match(/^([^:]+):(.*)$/);
    if (!match) {
      index += 1;
      continue;
    }
    const [, key, remainder] = match;
    if (remainder.trim() === '') {
      const child = parseYamlMapping(lines, index + 1, indent);
      value[key] = child.value;
      index = child.index;
    } else {
      value[key] = parseYamlScalar(remainder);
      index += 1;
    }
  }
  return { value, index };
}

function parseYamlDocument(yaml) {
  return parseYamlMapping(yaml.split(/\r?\n/), 0, -2).value;
}

function sortedRecord(value) {
  return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)));
}

function centralPermissionRequirements(jobs) {
  const levels = { none: 0, read: 1, write: 2 };
  const requirements = {};
  for (const [jobName, job] of Object.entries(jobs ?? {})) {
    if (job.permissions === undefined || job.permissions === '{}') continue;
    assert.equal(typeof job.permissions, 'object', `${jobName} permissions must be an explicit map`);
    for (const [scope, access] of Object.entries(job.permissions)) {
      assert.ok(access in levels, `${jobName} has invalid ${scope} permission ${access}`);
      if (levels[access] > levels[requirements[scope] ?? 'none']) {
        requirements[scope] = access;
      }
    }
  }
  return sortedRecord(requirements);
}

function findCentralCallerJob(callerDocument) {
  const matches = Object.entries(callerDocument.jobs ?? {}).filter(([, job]) =>
    typeof job.uses === 'string' &&
    job.uses.startsWith('Kizunad/review/.github/workflows/review.yml@'),
  );
  assert.equal(matches.length, 1, 'caller must contain exactly one central review reusable job');
  return matches[0][1];
}

function assertKnownCentralInterface(centralDocument) {
  const call = centralDocument.on?.workflow_call;
  assert.ok(call, 'central workflow must declare workflow_call');
  for (const [name, expected] of Object.entries(expectedCentralInputs)) {
    const actual = call.inputs?.[name];
    assert.ok(actual, `central workflow is missing input ${name}`);
    assert.equal(actual.type, expected.type, `${name} type changed`);
    assert.equal(actual.required, expected.required, `${name} required flag changed`);
    if (Object.hasOwn(expected, 'default')) {
      assert.equal(actual.default, expected.default, `${name} default changed`);
    } else {
      assert.ok(!Object.hasOwn(actual, 'default'), `${name} must not define a default`);
    }
  }
  assert.equal(call.secrets?.review_api_key?.required, true);
  assert.equal(centralDocument.permissions, '{}');
}

function assertReusableCompatibility(centralYaml, callerYaml) {
  const centralDocument = parseYamlDocument(centralYaml);
  const callerDocument = parseYamlDocument(callerYaml);
  const call = centralDocument.on?.workflow_call;
  assert.ok(call, 'central workflow must declare workflow_call');
  const callerJob = findCentralCallerJob(callerDocument);
  const centralInputs = call.inputs ?? {};
  const callerInputs = callerJob.with ?? {};
  const centralSecrets = call.secrets ?? {};
  const callerSecrets = callerJob.secrets ?? {};

  for (const name of Object.keys(callerInputs)) {
    assert.ok(centralInputs[name], `caller maps unknown central input ${name}`);
  }
  for (const [name, definition] of Object.entries(centralInputs)) {
    if (definition.required === true) {
      assert.ok(Object.hasOwn(callerInputs, name), `central workflow requires unmapped input ${name}`);
    }
  }
  for (const name of Object.keys(callerSecrets)) {
    assert.ok(centralSecrets[name], `caller maps unknown central secret ${name}`);
  }
  for (const [name, definition] of Object.entries(centralSecrets)) {
    if (definition.required === true) {
      assert.ok(Object.hasOwn(callerSecrets, name), `central workflow requires unmapped secret ${name}`);
    }
  }

  const requiredPermissions = centralPermissionRequirements(centralDocument.jobs);
  assert.deepEqual(
    sortedRecord(callerJob.permissions ?? {}),
    requiredPermissions,
    'caller permission ceiling must exactly match the central job requirements',
  );
  return centralDocument;
}

const centralContractFixture = `on:
  workflow_call:
    inputs:
      required_input:
        required: true
        type: string
      optional_input:
        required: false
        default: false
        type: boolean
    secrets:
      required_secret:
        required: true
permissions: {}
jobs:
  read:
    permissions:
      contents: read
      pull-requests: read
  publish:
    permissions:
      contents: read
      pull-requests: write
      issues: write
`;

const callerContractFixture = `permissions: {}
jobs:
  review:
    permissions:
      contents: read
      pull-requests: write
      issues: write
    uses: Kizunad/review/.github/workflows/review.yml@1111111111111111111111111111111111111111
    with:
      required_input: value
    secrets:
      required_secret: secret
`;

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
    /uses: Kizunad\/review\/\.github\/workflows\/review\.yml@417e55e55737b8fe42803b97f85b59fce8bbfb2a/,
  );
  assert.doesNotMatch(yaml, /Kizunad\/review\/[^\n]*@(main|master|v?\d|[0-9a-f]{1,39})\b/);
  assert.match(yaml, /pr_number: \$\{\{ fromJSON\(github\.event\.issue\.number \|\| inputs\.pr_number\) \}\}/);
  assert.match(yaml, /shadow: true/);
  assert.match(yaml, /worker_timeout_ms: 120000/);
  assert.match(yaml, /circuit_manual_retry: \$\{\{ github\.event_name == 'workflow_dispatch' \}\}/);
  assert.match(yaml, /policy_path: \.github\/review-policy\/bong\.v1\.json/);
  assert.match(yaml, /review_base_url: \$\{\{ vars\.REVIEW_CLAUDE_BASE_URL \|\| 'https:\/\/api\.claudeopus\.world' \}\}/);
});

test('shadow caller maps only the existing Claude credential and grants the central permission ceiling', async () => {
  const yaml = await workflow();
  assert.match(yaml, /actions: read/);
  assert.match(yaml, /contents: read/);
  assert.match(yaml, /pull-requests: write/);
  assert.match(yaml, /issues: write/);
  assert.match(yaml, /review_api_key: \$\{\{ secrets\.REVIEW_CLAUDE_API_KEY \}\}/);
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
  assert.match(yaml, /bubblewrap_0\.9\.0-1ubuntu0\.1_amd64\.deb/);
  assert.match(yaml, /1b506492bd9c7fd0cdb4f02ac822f1d3e336b0aead5113c1239baf8db5db562a/);
  assert.match(yaml, /sha256sum --check --strict/);
  assert.match(yaml, /root:root 4755/);
  assert.match(yaml, /BWRAP_EXECUTABLE=%s/);
  assert.match(yaml, /(?:\n\s+cd _central-contract\n\s+npm test\n)/);
  assert.doesNotMatch(yaml, /repository: Kizunad\/review[\s\S]*?ref: (?:main|master|v?\d|[0-9a-f]{1,39})\b/);
});

test('checked-out central workflow exactly matches the known interface and caller contract', async (context) => {
  const centralRoot = process.env.CENTRAL_REVIEW_CONTRACT_DIR;
  if (!centralRoot) {
    context.skip('CENTRAL_REVIEW_CONTRACT_DIR is required for the cross-repository contract check');
    return;
  }
  const centralYaml = await readFile(path.join(centralRoot, '.github/workflows/review.yml'), 'utf8');
  const callerYaml = await workflow();
  const centralDocument = assertReusableCompatibility(centralYaml, callerYaml);
  assertKnownCentralInterface(centralDocument);
});

test('contract gate rejects central required inputs and secrets missing from the caller', () => {
  const requiredInput = centralContractFixture.replace(
    '      optional_input:',
    '      added_required_input:\n        required: true\n        type: string\n      optional_input:',
  );
  assert.throws(
    () => assertReusableCompatibility(requiredInput, callerContractFixture),
    /requires unmapped input added_required_input/,
  );

  const requiredSecret = centralContractFixture.replace(
    'permissions: {}',
    '      added_required_secret:\n        required: true\npermissions: {}',
  );
  assert.throws(
    () => assertReusableCompatibility(requiredSecret, callerContractFixture),
    /requires unmapped secret added_required_secret/,
  );
});

test('contract gate permits new optional inputs and rejects unknown caller mappings', () => {
  const optionalInput = centralContractFixture.replace(
    '      optional_input:',
    '      added_optional_input:\n        required: false\n        default: 7\n        type: number\n      optional_input:',
  );
  assert.doesNotThrow(() => assertReusableCompatibility(optionalInput, callerContractFixture));

  const unknownInput = callerContractFixture.replace(
    '      required_input: value',
    '      required_input: value\n      removed_input: value',
  );
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, unknownInput),
    /maps unknown central input removed_input/,
  );

  const unknownSecret = callerContractFixture.replace(
    '      required_secret: secret',
    '      required_secret: secret\n      removed_secret: secret',
  );
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, unknownSecret),
    /maps unknown central secret removed_secret/,
  );
});

test('contract gate rejects missing mappings and any permission ceiling drift', () => {
  const missingInput = callerContractFixture.replace('      required_input: value\n', '');
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, missingInput),
    /requires unmapped input required_input/,
  );

  const missingSecret = callerContractFixture.replace('      required_secret: secret\n', '');
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, missingSecret),
    /requires unmapped secret required_secret/,
  );

  const insufficientPermission = callerContractFixture.replace('      pull-requests: write', '      pull-requests: read');
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, insufficientPermission),
    /permission ceiling must exactly match/,
  );

  const excessivePermission = callerContractFixture.replace(
    '      issues: write',
    '      issues: write\n      actions: read',
  );
  assert.throws(
    () => assertReusableCompatibility(centralContractFixture, excessivePermission),
    /permission ceiling must exactly match/,
  );
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
    /uses: Kizunad\/review\/\.github\/workflows\/provider-canary\.yml@[0-9a-f]{40}/,
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
  assert.equal(value.version, 'project-review-policy.v1');
  assert.equal(value.project, 'Kizunad/Bong');
  assert.equal(value.minorFindingsRequestChanges, true);
  assert.ok(value.rules.length >= 20 && value.rules.length <= 256);

  const ids = new Set();
  for (const rule of value.rules) {
    assert.deepEqual(Object.keys(rule).sort(), ['id', 'severity', 'text']);
    assert.match(rule.id, /^[a-z][a-z0-9-]{0,79}$/);
    assert.ok(!ids.has(rule.id), `duplicate policy rule: ${rule.id}`);
    ids.add(rule.id);
    assert.ok(['blocker', 'major', 'minor'].includes(rule.severity));
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
  ]) {
    assert.ok(ids.has(required), `missing Bong policy rule: ${required}`);
  }

  const serialized = JSON.stringify(value);
  assert.match(serialized, /醒灵.*引气.*凝脉.*固元.*通灵.*化虚/);
  assert.match(serialized, /骨币/);
  assert.match(serialized, /SPIRIT_QI_TOTAL/);
  assert.match(serialized, /LAYER_REGISTRY/);
  assert.doesNotMatch(serialized, /(?:^|\W)(?:command|shell|runner|action ref|MCP server)(?:\W|$)/i);
});
