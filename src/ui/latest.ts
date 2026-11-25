/**
 * File: ui/latest.ts
 * Purpose: The one "latest request wins" helper: a slow reply to an older request never replaces the newer one's.
 * Contents: latest — a request counter.
 */

/** A request counter for one stream of requests. Return: a function to call once per request, which returns that request's check (true while no newer request has started) carrying the request's number as `seq`, for a backend that orders requests too. */
export function latest() {
  let n = 0;
  return () => {
    const mine = ++n;
    return Object.assign(() => mine === n, { seq: mine });
  };
}
