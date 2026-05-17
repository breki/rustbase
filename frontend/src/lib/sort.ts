// `undefined` locale lets Intl pick the runtime default;
// pass an explicit locale string to override per call site.
function makeCollator(
  locale: string | undefined,
  numeric: boolean,
): Intl.Collator {
  return new Intl.Collator(locale, { sensitivity: "base", numeric });
}

const DEFAULT_NAME_COLLATOR = makeCollator(undefined, false);
const DEFAULT_ID_COLLATOR = makeCollator(undefined, true);

export function compareNames(
  a: string | null | undefined,
  b: string | null | undefined,
  locale?: string,
): number {
  const collator =
    locale === undefined ? DEFAULT_NAME_COLLATOR : makeCollator(locale, false);
  return collator.compare(a ?? "", b ?? "");
}

export function compareIds(
  a: string | null | undefined,
  b: string | null | undefined,
  locale?: string,
): number {
  const collator =
    locale === undefined ? DEFAULT_ID_COLLATOR : makeCollator(locale, true);
  return collator.compare(a ?? "", b ?? "");
}
