# Transport security headers

Browser-facing endpoints need a stack of HTTP response headers that
mitigate categories of attacks the browser itself can defend against
*if* we tell it to. None of these headers are difficult to set; the
discipline is in setting *all* of them, picking the right value, and
not relaxing the value over time.

This document is the per-header reference: what it does, the value
MemberClub should ship, and how to test it.

The intended deployment point is a single `SetResponseHeaderLayer`
chain in `apps/memberclub/api`. Wire it in at the outermost router
layer so even error responses carry the headers.

---

## Content-Security-Policy (CSP)

**What it does.** Tells the browser which sources of script, style,
image, font, and frame content are legitimate. Blocks inline scripts,
inline event handlers, and `eval()` unless explicitly allowed. The
single most effective XSS mitigation that exists.

**The value MemberClub should ship.**

```
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'nonce-{nonce}';
  style-src 'self' 'nonce-{nonce}';
  img-src 'self' data: https://images.memberclub.com;
  font-src 'self';
  connect-src 'self' https://api.memberclub.com https://api.stripe.com;
  frame-src https://js.stripe.com https://hooks.stripe.com;
  object-src 'none';
  base-uri 'self';
  form-action 'self';
  frame-ancestors 'none';
  upgrade-insecure-requests;
  report-uri /csp-report;
```

Notes:

- `nonce-{nonce}` is a per-response random value; emitted by the
  Svelte renderer onto every legitimate inline `<script>` and
  `<style>` tag. The middleware generates it, attaches it to the
  request context, and substitutes into the header.
- `frame-src` allows Stripe Elements and Checkout.
- `frame-ancestors 'none'` prevents the site from being framed —
  belt to `X-Frame-Options`'s suspenders.
- `report-uri` collects violations; pipe the endpoint into your log
  pipeline. The first week after deploy will surface every
  third-party tracker you don't know about.

**Rollout.** Ship `Content-Security-Policy-Report-Only` first; watch
the reports; fix violations; flip to the enforcing header. Going
straight to enforcing breaks pages.

**Test.**

```sh
curl -sI https://memberclub.com | grep -i content-security-policy
# Then in the browser: open devtools, watch the Console for CSP
# violation messages on every page.
```

---

## Strict-Transport-Security (HSTS)

**What it does.** Tells the browser "for the next N seconds, never
talk to this hostname over plain HTTP, even if the user types
`http://`." Eliminates the SSL-stripping man-in-the-middle attack on
return visits.

**The value MemberClub should ship.**

```
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

- `max-age=31536000` — one year.
- `includeSubDomains` — covers `api.memberclub.com`,
  `app.memberclub.com`, etc.
- `preload` — opts into the [HSTS preload
  list](https://hstspreload.org/) shipped in browsers, so even first
  visits go over HTTPS.

**The catch.** `includeSubDomains` plus `preload` is a one-way door.
If you have any internal subdomain that doesn't speak HTTPS, this
will break it permanently for browsers until the cache expires.
Audit all `*.memberclub.com` before flipping.

**Test.**

```sh
curl -sI https://memberclub.com | grep -i strict-transport-security
```

For the preload check, submit at hstspreload.org and wait for the
status email.

---

## X-Content-Type-Options

**What it does.** Disables MIME-type sniffing. Without it, a browser
that sees `Content-Type: text/plain` on a response containing what
looks like a script may execute it as a script — a classic vector for
serving user-uploaded content from the same origin.

**The value MemberClub should ship.**

```
X-Content-Type-Options: nosniff
```

There is no other value. Set it on every response.

**Test.**

```sh
curl -sI https://memberclub.com/anything | grep -i x-content-type-options
```

---

## X-Frame-Options

**What it does.** Tells the browser "do not load this response inside
a frame / iframe." Blocks clickjacking, where an attacker embeds our
site inside theirs and overlays an invisible button on top of one of
our buttons.

**The value MemberClub should ship.**

```
X-Frame-Options: DENY
```

`DENY` is stricter than `SAMEORIGIN` — even our own marketing site
can't iframe `app.memberclub.com`. If a legitimate use case appears,
loosen to `SAMEORIGIN` rather than allowlisting specific origins.

Note: `X-Frame-Options` is deprecated in favor of CSP's
`frame-ancestors`. Ship both — older browsers honor only the legacy
header.

**Test.**

```sh
curl -sI https://app.memberclub.com | grep -i x-frame-options
```

---

## Referrer-Policy

**What it does.** Controls how much of the current URL is sent in the
`Referer` header when the user follows a link off-site. Default
browser behavior leaks the full URL, including query strings — which
often contain session ids, search terms, or PII.

**The value MemberClub should ship.**

```
Referrer-Policy: strict-origin-when-cross-origin
```

- Same-origin requests get the full URL (we need it for analytics).
- Cross-origin requests over HTTPS get only the origin
  (`https://memberclub.com`), no path or query.
- HTTPS → HTTP requests get nothing (no leak through a downgrade).

This is the modern default in Chrome but explicit-is-better-than-
implicit.

**Test.**

```sh
curl -sI https://memberclub.com | grep -i referrer-policy
```

---

## Permissions-Policy

**What it does.** Replaces the older `Feature-Policy`. Tells the
browser which JavaScript APIs are allowed for this document and
embedded frames — camera, microphone, geolocation, USB, payment, etc.

**The value MemberClub should ship.**

```
Permissions-Policy:
  camera=(),
  microphone=(),
  geolocation=(),
  usb=(),
  payment=(self "https://js.stripe.com"),
  interest-cohort=()
```

- Empty `()` = no origin allowed. Disables the feature entirely.
- `payment=(self "https://js.stripe.com")` allows the Payment Request
  API on our origin and Stripe's frame; nothing else.
- `interest-cohort=()` opts out of FLoC / Topics tracking.

When a new API ships in browsers, add it to the policy with `()` by
default. Allowlist explicitly when a feature needs it.

**Test.**

```sh
curl -sI https://memberclub.com | grep -i permissions-policy
```

In the browser, attempting to call a denied API throws a
`SecurityError`.

---

## Subresource-Integrity (SRI)

**What it does.** Not a response header — a per-tag attribute
(`integrity="sha384-..."`) on `<script>` and `<link>` tags that load
from a third-party CDN. The browser computes the hash of the fetched
asset and refuses to execute it if the hash doesn't match. Catches a
compromised CDN serving altered JS.

**What MemberClub should ship.**

- Every `<script src="https://...">` tag pointing off-origin must
  have `integrity` set.
- Every `<link rel="stylesheet" href="https://...">` tag pointing
  off-origin must have `integrity` set.
- Pair with `crossorigin="anonymous"` so the browser actually
  validates (without `crossorigin` the integrity check is skipped on
  cross-origin requests).

Example:

```html
<script
  src="https://js.stripe.com/v3/"
  integrity="sha384-CHANGE-ME-ON-EVERY-RELEASE"
  crossorigin="anonymous"></script>
```

The hash must be regenerated whenever the upstream asset changes —
Stripe.js doesn't version-lock, so SRI on Stripe.js is operationally
difficult. The compromise: pin to a specific versioned URL when the
vendor offers one (`https://js.stripe.com/v3/`), and accept that an
upstream compromise of an unpinned URL would slip past.

**Test.** Open devtools, find the script tag, alter the `integrity`
attribute by one character, reload — the browser refuses to execute
and logs an SRI failure to the console.

---

## Deployment summary

The full header set, ready to paste into `SetResponseHeaderLayer`:

```
Content-Security-Policy:  default-src 'self'; script-src 'self' 'nonce-{n}'; ...
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
X-Content-Type-Options:    nosniff
X-Frame-Options:           DENY
Referrer-Policy:           strict-origin-when-cross-origin
Permissions-Policy:        camera=(), microphone=(), geolocation=(), payment=(self), interest-cohort=()
```

End-to-end test in CI: a smoke test that fetches `/` from the deployed
app and asserts every header above is present with the expected
value. Drift detection — when someone "temporarily" relaxes a header,
the test fails.

Online graders to run against staging:

- [securityheaders.com](https://securityheaders.com) — grade A+ is the
  goal
- [observatory.mozilla.org](https://observatory.mozilla.org) — second
  opinion, gives detailed advice
