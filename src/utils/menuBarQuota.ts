import type { Account, QuotaData } from "../types/account";

export interface CompactQuotaRow {
  id: string;
  label: string;
  window?: string;
  remaining: number | null;
  resetTime: string;
}
export interface CompactQuotaGroup {
  name: string;
  rows: CompactQuotaRow[];
}

export function validPercentage(
  value: unknown,
  fraction = false,
): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const scaled = fraction ? value * 100 : value;
  return scaled >= 0 && scaled <= 100 ? Math.round(scaled) : null;
}

/** Independent pools stay independent. Unknown data never becomes an empty bar. */
export function compactQuotaGroups(
  quota?: QuotaData,
  pinned: string[] = [],
): CompactQuotaGroup[] {
  if (!quota || quota.is_forbidden) return [];
  const groups = quota.quota_groups
    ?.filter((group) => group.buckets?.length)
    .map((group, groupIndex) => ({
      name: group.display_name,
      rows: group.buckets.map((bucket, index) => ({
        id: `${groupIndex}-${bucket.bucket_id || index}`,
        label: bucket.display_name || bucket.window,
        window: bucket.window,
        remaining: validPercentage(bucket.remaining_fraction, true),
        resetTime: bucket.reset_time,
      })),
    }));
  if (groups?.length) return groups;
  const models = quota.models || [];
  const selected = pinned.length
    ? models.filter((model) => pinned.includes(model.name))
    : [];
  return (selected.length ? selected : models).map((model) => ({
    name: model.display_name || model.name,
    rows: [
      {
        id: model.name,
        label: model.display_name || model.name,
        remaining: validPercentage(model.percentage),
        resetTime: model.reset_time,
      },
    ],
  }));
}

export function lowestKnownQuota(account: Account): number | null {
  const known = compactQuotaGroups(account.quota)
    .flatMap((group) => group.rows)
    .map((row) => row.remaining)
    .filter((v): v is number => v !== null);
  return known.length ? Math.min(...known) : null;
}

export function isAccountSwitchable(
  account: Account,
  now = Date.now(),
): boolean {
  const blocked =
    account.validation_blocked &&
    (!account.validation_blocked_until ||
      account.validation_blocked_until * 1000 > now);
  return !account.disabled && !blocked;
}

export function isQuotaStale(
  timestamp?: number,
  intervalMinutes = 15,
  now = Date.now(),
): boolean {
  return (
    !timestamp ||
    !Number.isFinite(timestamp) ||
    now - timestamp * 1000 > Math.max(intervalMinutes * 60_000, 60_000)
  );
}

export function resetTimestamp(value: string): number | null {
  if (!value) return null;
  const valueMs = Date.parse(value);
  return Number.isFinite(valueMs) ? valueMs : null;
}

export interface PagedQuotaRow extends CompactQuotaRow {
  group: string;
}
/** Bounded pages keep every data item accessible without an unbounded popover. */
export function quotaPages(
  groups: CompactQuotaGroup[],
  pageSize = 4,
): PagedQuotaRow[][] {
  const size = Math.max(1, Math.floor(pageSize));
  const rows = groups.flatMap((group) =>
    group.rows.map((row) => ({ ...row, group: group.name })),
  );
  return Array.from({ length: Math.ceil(rows.length / size) }, (_, index) =>
    rows.slice(index * size, (index + 1) * size),
  );
}

export function pageSlice<T>(items: T[], page: number, pageSize = 4): T[] {
  const size = Math.max(1, Math.floor(pageSize));
  const lastPage = Math.max(0, Math.ceil(items.length / size) - 1);
  const index = Math.max(0, Math.min(Math.floor(page), lastPage));
  return items.slice(index * size, (index + 1) * size);
}

export function pageSizeForHeight(height: number): number {
  if (height >= 440) return 4;
  if (height >= 360) return 3;
  if (height >= 300) return 2;
  return 1;
}
