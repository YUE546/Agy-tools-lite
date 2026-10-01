import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
const source = ts.transpileModule(
  readFileSync(
    new URL("../src/utils/menuBarQuota.ts", import.meta.url),
    "utf8",
  ),
  {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  },
).outputText;
const {
  compactQuotaGroups,
  validPercentage,
  lowestKnownQuota,
  isAccountSwitchable,
  isQuotaStale,
  resetTimestamp,
} = await import(
  `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
);
let passed = 0;
const test = (name, check) => {
  check();
  passed++;
  console.log(`PASS ${name}`);
};
test("unknown percentages remain unknown", () => {
  for (const value of [undefined, null, NaN, Infinity, -1, 101, "50"])
    assert.equal(validPercentage(value), null);
});
test("zero is a real known quota", () => assert.equal(validPercentage(0), 0));
test("fraction must lie within its actual range", () => {
  assert.equal(validPercentage(0.42, true), 42);
  assert.equal(validPercentage(1.2, true), null);
});
test("missing groups fall back to reported models only", () =>
  assert.deepEqual(
    compactQuotaGroups(
      { models: [{ name: "reported", percentage: 70, reset_time: "" }] },
      ["missing"],
    )[0].rows[0].remaining,
    70,
  ));
test("pinned model order is filtered without fabricating zeros", () =>
  assert.equal(
    compactQuotaGroups(
      {
        models: [
          { name: "a", percentage: 0, reset_time: "" },
          { name: "b", percentage: 50, reset_time: "" },
        ],
      },
      ["a"],
    ).length,
    1,
  ));
test("pool windows remain separate", () => {
  const groups = compactQuotaGroups({
    models: [],
    quota_groups: [
      {
        display_name: "Gemini",
        buckets: [
          {
            bucket_id: "1",
            window: "5h",
            remaining_fraction: 0.8,
            reset_time: "",
          },
          {
            bucket_id: "2",
            window: "weekly",
            remaining_fraction: 0.2,
            reset_time: "",
          },
        ],
      },
    ],
  });
  assert.equal(groups[0].rows.length, 2);
  assert.equal(groups[0].rows[1].remaining, 20);
});
test("forbidden quota never renders stale percentages", () =>
  assert.deepEqual(
    compactQuotaGroups({
      is_forbidden: true,
      models: [{ name: "a", percentage: 70 }],
    }),
    [],
  ));
test("no data produces no card", () =>
  assert.deepEqual(compactQuotaGroups(undefined), []));
test("lowest reported balance does not average independent pools", () =>
  assert.equal(
    lowestKnownQuota({
      quota: {
        models: [
          { name: "a", percentage: 50 },
          { name: "b", percentage: 10 },
        ],
      },
    }),
    10,
  ));
test("unknown account quota is not zero", () =>
  assert.equal(lowestKnownQuota({}), null));
test("disabled and currently blocked accounts cannot switch", () => {
  assert.equal(isAccountSwitchable({ disabled: true }), false);
  assert.equal(isAccountSwitchable({ validation_blocked: true }), false);
  assert.equal(
    isAccountSwitchable(
      { validation_blocked: true, validation_blocked_until: 1 },
      2000,
    ),
    true,
  );
});
test("quota freshness and missing timestamps", () => {
  assert.equal(isQuotaStale(undefined), true);
  assert.equal(isQuotaStale(100, 15, 200000), false);
  assert.equal(isQuotaStale(100, 15, 2000000), true);
});
test("invalid resets stay unavailable", () => {
  assert.equal(resetTimestamp("bad"), null);
  assert.equal(resetTimestamp(""), null);
  assert.ok(resetTimestamp("2026-10-01T12:00:00Z"));
});
console.log(`${passed} tests passed`);
