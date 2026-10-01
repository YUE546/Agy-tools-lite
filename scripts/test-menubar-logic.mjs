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
  quotaPages,
  pageSlice,
  pageSizeForHeight,
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
test("quota pages show at most four rows without dropping later pools", () => {
  const groups = Array.from({ length: 3 }, (_, index) => ({
    name: `Pool ${index}`,
    rows: Array.from({ length: 3 }, (_, row) => ({
      id: `${index}-${row}`,
      label: "weekly",
      remaining: 50,
      resetTime: "",
    })),
  }));
  const pages = quotaPages(groups);
  assert.deepEqual(
    pages.map((page) => page.length),
    [4, 4, 1],
  );
  assert.equal(pages.flat().length, 9);
  assert.equal(pages[2][0].group, "Pool 2");
});
test("model fallback is paginated rather than silently truncated", () => {
  const groups = compactQuotaGroups({
    models: Array.from({ length: 7 }, (_, index) => ({
      name: `model-${index}`,
      percentage: 50,
      reset_time: "",
    })),
  });
  assert.deepEqual(
    quotaPages(groups).map((page) => page.length),
    [4, 3],
  );
});
test("large account lists use fixed size pages and clamp stale page indexes", () => {
  const accounts = Array.from({ length: 13 }, (_, index) => index);
  assert.deepEqual(pageSlice(accounts, 0), [0, 1, 2, 3]);
  assert.deepEqual(pageSlice(accounts, 3), [12]);
  assert.deepEqual(pageSlice(accounts, 999), [12]);
  assert.deepEqual(pageSlice(accounts, -4), [0, 1, 2, 3]);
  assert.deepEqual(pageSlice([], 4), []);
});
test("the compact panel has no scroll surfaces or decorative gradients", () => {
  const css = readFileSync(
    new URL("../src/components/menubar/MenuBarDashboard.css", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(css, /overflow(?:-y)?:\s*(auto|scroll)/);
  assert.doesNotMatch(css, /(?<!repeating-)linear-gradient/);
  assert.match(css, /overflow:\s*hidden/);
});
test("short display work areas reduce page size instead of scrolling or clipping", () => {
  assert.equal(pageSizeForHeight(480), 4);
  assert.equal(pageSizeForHeight(400), 3);
  assert.equal(pageSizeForHeight(330), 2);
  assert.equal(pageSizeForHeight(280), 1);
});
console.log(`${passed} tests passed`);
