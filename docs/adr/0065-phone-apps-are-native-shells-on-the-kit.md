# ADR-0065: The phone apps are Capacitor shells on the kit, each in its platform's look

- Status: accepted (each open question has a default that holds until the owner answers)
- Date: 2026-11-14
- Source: the owner, 2026-11-13 (docs/v2-queue.md item 8): "Android and iPhone don't copy v1's look. Each follows its
  platform (Material 3; Apple HIG), built from `@wisp/ui` kit components with per-platform variants on wisp's tokens,
  no app CSS; every v1 feature kept." v1: `mobile/` (Capacitor 8.5.3, Android only; `mobile/README.md`),
  `web/js/features/{notify,share-in,cache,homescreen}.js`, `web/sw.js`, `grid/server.js` device routes.
  v1 rows: F-phone-1..8, F-stuff-10, F-share-4, F-set-6 (docs/v1-features.md).
- With ADR-0010 (peers and transports; "the apps ship the bundle"), ADR-0013 (Rust except the UI), ADR-0016
  (v2 looks like v1), ADR-0017 (immediate feedback), ADR-0012 (nothing wraps), ADR-0020 and ADR-0036 (My Stuff apps
  in a sandboxed frame), ADR-0031 (no v1 compat), ADR-0040 (meet users where they work), ADR-0061 (several grid
  accounts). The design is `design/mobile.md` (Part A).
- **Supersedes ADR-0016 for the phone apps only:** a view rendered with `data-platform=android|ios` follows
  `design/mobile.md`, not `design/v1-reference/`. The browser at any width, phones included, stays on ADR-0016.

## Context

- v1's Android app is a Capacitor 8.5.3 shell with about 2,100 lines of Java (`mobile/android/.../dev/trywisp/mobile/`):
  sshj 0.41.2 + BouncyCastle tunnels, a `specialUse` foreground service, local notifications posted by the page,
  the share sheet (`Shared.java`), camera and dictation. Its WebView loads the computer's own page through the
  tunnel. The debug APK is 7.96 MB (minify off). minSdk 26, targetSdk 36. There is no iOS app: iPhone users get
  the Home Screen web app on a Grid, with alerts from ntfy only.
- v1's own "Not yet" list (`mobile/README.md`): notifications stop when the app is swiped away, keys are files and
  not in the Keystore, host keys are trusted on first use with a manual Forget, one computer at a time (one local
  port), no iOS. The device token is kept in plain `localStorage`.
- v2 already has the pieces the app's page needs: `mobile/web` (`@wisp/mobile`: `bridge.ts` types the native side,
  `transport.ts` makes each saved computer a `@wisp/client` transport, `migrate.ts` copies v1's computers),
  `@wisp/client` with N peers, persisted rows, cursors and pending commands (`persist.ts`), and the shell's
  `platform` module (`packages/shell/src/platform/`, kinds `browser | desktop | android`).
- The owner wants the apps to feel native, not like v1 in a WebView, while every feature stays and the web app at a
  phone's width keeps v1 parity.

## Decision

### 1. The shell: Capacitor on both platforms, the bundle inside

| | Capacitor (chosen) | Tauri 2 mobile | A thin native WebView shell |
|---|---|---|---|
| Build size | v1: 7.96 MB debug APK unminified; Capacitor's runtime is a small part of it (sshj + BouncyCastle are most of the native weight). Budget below. | Adds a Rust library per ABI (arm64, armv7, x86_64 for Android; arm64 for iOS) on top of the WebView. Not measured here; it can only be larger than Capacitor's Java/Swift runtime for the same features. | Smallest runtime, but we'd write the bridge, plugin loading, deep links and lifecycle ourselves. |
| Push | `@capacitor/push-notifications` (FCM, APNs) maintained by the Capacitor team; or our own Kotlin/Swift, as now. | No first-party push plugin; ours to write in Kotlin and Swift anyway. | Ours to write. |
| Share sheet in | An intent filter (Android, `Shared.java` today) and a Share Extension target (iOS); both native code in any shell. | Same native code, plus Tauri's plugin glue. | Same native code. |
| Background | Android foreground service (`TunnelService.java` exists); iOS gives no long-lived sockets in any shell (§4). | Same OS limits; the service is Kotlin either way. | Same. |
| SSH | sshj in Java exists and is tested (`KeysTest`, `TunnelTest`); Citadel (SwiftNIO SSH) on iOS. | russh in Rust would serve both, but means a rewrite of working Java, cross-compiling Rust for 4 mobile targets in CI and UniFFI-style bindings. | Same as Capacitor. |
| Store review | Capacitor apps pass both stores routinely; v1 is on Play already (closed test, Data safety, FGS video done). | Fine in principle; newer on mobile, fewer precedents. | Fine. |
| What it costs us | Keep what we have, add iOS. | Rewrite the Android app and its tests. | Rewrite the bridge Capacitor gives for free. |

So:
- **`mobile/` stays one Capacitor project with two platforms:** `mobile/android/` (Java, kept) and `mobile/ios/`
  (Swift, new). The pinned Capacitor major is v1's (8). Upgrading it is its own PR.
- **The app ships the v2 bundle** (`apps/web`'s build plus `@wisp/mobile`) inside the APK/IPA, as ADR-0010 says; the
  first paint never waits on a tunnel. It talks to every computer over the sync protocol (SSE + `POST /api/cmd`).
  It never loads a computer's own page (v1 did).
- **One native contract for both platforms:** `mobile/web/src/bridge.ts` (`NativeBridge`) is the spec. Android's
  `WispSshPlugin` and iOS's `WispSshPlugin.swift` implement it, and one TypeScript contract suite
  (`mobile/web/test/bridge-contract.test.ts`) runs against each platform's recorded fake. Native code stays glue: SSH,
  keys, push, share, capture, dictation, haptics, back. **No feature logic in Java or Swift** (the kernel rule that
  no feature is named outside its plugin holds: the shell has no `if task…`).
- **New dependencies** (pinned in `mobile/ios/` and justified here): Citadel (SwiftNIO SSH, MIT) for iOS SSH; on the
  wisp side the `hpke` crate (RFC 9180) for sealing push payloads (§3); on Android, Firebase Messaging only in the
  `play` flavour (§3). Nothing else.
- **Size budgets** (`tools/budgets.ts`, ADR-0011): Android release AAB download ≤ 8 MB (R8 on), iOS App Store
  download ≤ 15 MB. A miss blocks the merge like the other budgets.

### 2. How the app reaches a wisp

Every way is a saved **computer** (`bridge.ts` `Computer`), reached through a transport; the app holds several at
once (v1 held one).

1. **A computer over SSH** (F-phone-1): `you@host[:port]`, the password used once to add the phone's key, then the
   key. Tailscale SSH tried first (v1). The bootstrap (the v2 one that installs the `wisp` binary) starts wisp; the
   phone forwards a free local port per computer to its `127.0.0.1:<port>` and authenticates with the computer's
   own token (`server.json`, read over SSH).
   - **Keys live in the Android Keystore and the iOS Keychain**, never in files or web storage. v1's key files and
     `localStorage` tokens move there once, on the first v2 start (`migrate.ts` + the native side), then are deleted.
   - **Host keys are pinned, not trusted on first use, when they can be:** wisp on the desktop shows *Add to your
     phone* as a QR code (`wisp+ssh://you@host:port?fp=SHA256:…&name=Studio`); scanning it pins the fingerprint before
     the first connect. Typed by hand, the first key is shown and kept (v1's TOFU). A changed key is refused with
     *This computer's key changed* and a *Trust the new key* button behind a confirmation (v1 needed a hidden Forget).
2. **On the same network:** the same SSH, with the address offered from Bonjour/mDNS (`_ssh._tcp` and `_sftp-ssh._tcp`
   on the LAN; iOS asks for Local Network once). wisp never listens beyond loopback for the phone: there is no LAN
   HTTP pairing (it would put wisp's API on the network with a long-lived token).
3. **wisp Cloud** (F-phone-5): one tap, *Continue with GitHub* / *Google* (and *Apple* on iOS, §6). The device flow
   is v1's (RFC 8628: `/api/device/code`, the system browser at `/auth/<provider>/start`, `/activate`, poll
   `/api/device/token`): Custom Tabs on Android, `ASWebAuthenticationSession` on iOS, back through
   `dev.trywisp.mobile://signed-in`. Several accounts, one card each (ADR-0061). The cloud box and **your computers on
   the grid** (grid-tunnel) are listed from `GET /api/computers`; each is a `GridComputer` with the device token,
   reached over HTTPS with no SSH. A passive subscription never wakes a box (ADR-0010; `gridAsleep` in
   `transport.ts`): it shows *Asleep* with *Wake*.
4. **Your own wisp Grid:** behind *Use your own wisp Grid*, the same flow at another address.

Tokens (device tokens, computer tokens) are in the Keystore/Keychain, handed to the page per connect
(`NativeBridge.connect`, whose `Reached` result carries them), and never written to `localStorage` or IndexedDB.
This changes `bridge.ts`: `GridComputer.token` leaves the saved `Computer` and only comes back in `Reached`.

### 3. Notifications: wisp seals, the grid relays, the phone opens

v1 posted notifications from its page while the app ran, so they stopped when it was swiped away. v2 sends them the
way chat already gets them (ADR-0037): **a channel transport, `push`, in a new plugin `wisp.push`** (`plugins/push`,
like `wisp.ntfy`; a `channel` resource with `transport: push`). No new kernel point: it fills `wisp.channels`'
existing transport point. ADR-0037 keeps one channel per transport, so there is **one `push` channel per wisp**; its
devices (several phones) are a list in its secrets, one entry each, and unpairing a phone removes its entry.

1. **Pairing.** When the app connects to a computer, it asks the relay for a push handle (an opaque id the relay
   maps to the FCM or APNs token) and makes an X25519 key pair in the Keystore/Keychain. It then runs
   `push.pair {handle, publicKey, platform, device}` on that wisp, which adds the device to the `push` channel and
   keeps the handle in secrets (`~/.wisp/secrets.json`), never in the store. `push.pair` and `push.unpair` are named
   by the phone's capability set only and marked `ui-only` (reason: the phone's own keys pair it; no harness has a
   device to pair). `push.devices` has MCP exposure, so an agent can say which phones get alerts.
2. **Sending.** When a task finishes, asks or waits for approval (v1's three triggers, the same decision
   `wisp.channels` already makes for ntfy), wisp seals `{title, body, target, choices}` to the device's key with HPKE
   (RFC 9180, X25519/HKDF-SHA256/ChaCha20-Poly1305) and posts `{handle, sealed, collapse: <task id>}` to the relay.
3. **The relay** is a route on wisp Grid (`POST /api/push/register`, `POST /api/push/send`): it holds the FCM and
   APNs credentials, maps the handle, rate-limits per handle, and forwards a data message (FCM) or a
   `mutable-content` alert (APNs) whose visible fallback is a fixed *A task needs you* (shown if the extension can't
   decrypt in time). It sees only ciphertext, a handle and a collapse key. Google and Apple see the same.
4. **The phone** decrypts in a `FirebaseMessagingService` (Android) or a Notification Service Extension (iOS),
   posts one notification per task (the collapse key replaces the older one, as `NotifyOptions.tag` does today),
   and a tap opens the task (`target`). A question's choices (at most 3, like ntfy's buttons) are notification
   actions that send the answer through the same signed path as the app's own buttons; *Reply* offers free text
   (RemoteInput / `UNTextInputNotificationAction`).
5. **Without the relay:**
   - Android without Google Play services: UnifiedPush (ntfy is a distributor) in the `foss` flavour; without one,
     v1's way: the foreground service holds the tunnel and the page posts local notifications.
   - Any platform: the ntfy and chat channels keep working as today.
   - The badge is the need-you count, set by the push (`badge`) and by the page when open.
6. **Foreground service (Android):** only while an SSH computer without a push channel is connected, and only with
   the user's *Stay connected* switch (v1 had it on whenever connected). Type `specialUse` as v1 (declared on Play).
   With push paired, the tunnel closes 5 minutes after the app goes to the background.

### 4. Offline and a bad connection (F-phone-8)

- **First paint from the phone:** the bundle is in the app; rows, cursors and drafts come from IndexedDB
  (`persist-idb.ts`) for every saved computer. Budget: the list of 4,000 cached tasks painted ≤ 800 ms after a cold
  start on the reference phone (Pixel 6a class, Android; iPhone 12 class, iOS), with no network. The list is a
  `VirtualList` (4,000 rows, one row repaints).
- **Resume, never resync:** each peer reconnects from its cursor (`link` namespace); the server sends only what
  changed. A stream that stays silent past two keepalives is dropped and reopened with backoff (v1's 2/5/15/30/60 s);
  *pull to refresh* reopens it now, skipping the wait.
- **Writes work offline:** a command applies its optimistic patch at once and waits in `pending` (persisted, with its
  idempotency key) until its peer is back; the row shows *Waiting to send* in its fixed status column. Sent once on
  reconnect; a later refusal rolls back with a toast (ADR-0017).
- **Reads that need the network** (a transcript not cached, a diff) show their skeleton and an inline *Retry*; the
  last N opened tasks' transcripts are kept (v1 kept open tasks under 300 KB each and 1 MB more; v2 keeps the same
  budget in IndexedDB).
- **iOS in the background:** sockets are closed by the OS within about 30 s; the app closes its SSE streams when it
  goes to the background and resumes from the cursors on return. Notifications come from push (§3), not a socket.
- **Version skew:** the app's bundle and a computer's wisp may differ. The sync hello says the protocol version; a
  wisp too old for the app says *Update wisp on Studio* with the command, rather than half working. Unknown blocks
  render as `Unknown` (as today).

### 5. Look: kit variants by platform, and what leaves `qa/parity.mjs`

- The shell sets `data-platform="android"` or `"ios"` on `<html>` from `platform.kind`. **The browser never sets
  it**, at any width.
- **`Platform` grows** (`packages/shell/src/platform/types.ts`, additive, a contract change): `'ios'` in
  `PlatformKind`; `safeArea()` (insets, the gesture bar); `windowLayout()` (foldable hinges, size class);
  `openInApp(url)` (Custom Tabs / `SFSafariViewController` for `preview`); `onBack(progress)` (predictive and
  interactive Back). `haptic` exists already (`Haptic`: tap, select, success, warning, error).
- **Only the kit reads it.** Kit CSS has a variant per component under `:root[data-platform=…]` in `@layer kit`, and
  components that differ in structure (a FAB on Android, a compose button in the navigation bar on iOS) read it from
  the kit's `usePlatform()`. `tools/check` refuses `data-platform` and `platform.kind === 'android' | 'ios'` outside
  `packages/ui` and `packages/shell/src/platform` (rule `platform-branch`). Features and plugin UIs don't change: the
  same kit call renders Material 3 on Android and HIG on iOS.
- **Tokens:** platform token sets in `@wisp/tokens` (list in `design/mobile.md` §A.8), derived from wisp's colours.
  Their values come from the Material 3 and HIG specs, cited per token; features still never pick a number.
- **Out of `qa/parity.mjs`:** every view rendered with `data-platform` set. Parity keeps every browser view, the
  390 px and 320 px phone widths included.
- **What checks the apps instead:** `qa/layout.mjs` and `qa/feedback.mjs` run on the phone views too (at text scale
  100 % and 200 %), and a new **`qa/platform.mjs`** checks each view against `design/mobile.md`'s metric table
  (touch targets ≥ 48 dp / 44 pt, bar heights, safe areas, type roles, nothing hover-only, every gesture's visible
  twin present) at 360×800, 412×915 (Android), 375×667, 393×852, 440×956 (iOS) and 840 wide (tablet), light and dark.
  The native code has JUnit (`mobile/android/app/src/test`) and XCTest (`mobile/ios/App/AppTests`) suites in CI.

### 6. Store review, decided up front

- **Both:** account deletion reachable in the app (v1's quiet link under *Sign out*, opening `/account#delete`); a
  privacy policy (`trywisp.dev/privacy.html`); no executable code downloaded outside the WebView (My Stuff apps and
  plugin UIs are web content in the sandboxed frame, ADR-0036).
- **Play:** targetSdk follows Play's yearly floor; `specialUse` FGS declared with its video; POST_NOTIFICATIONS asked
  in context (the first time a task is started from the phone), not on launch; Data safety updated for push.
- **App Store:** an app offering Google or GitHub sign-in must also offer an equivalent privacy-preserving login
  (guideline 4.8; Sign in with Apple is the usual one), so the grid gets Apple sign-in before the iOS app ships (an *Open question* for the owner). Background modes:
  `remote-notification` only (no VoIP, no audio tricks). `NSLocalNetworkUsageDescription` for §2.2. Camera,
  microphone and speech strings for capture and dictation.

## Consequences

- One project, two native layers; the web bundle, the kit and the client are shared with the browser and desktop.
  A feature built once shows on both phones in their look, with no phone-specific feature code.
- The kit grows a variant per component and a platform token set; `packages/ui` gets bigger, but only the phone
  bundle's CSS carries the phone variants (split by `data-platform` at build), so the browser's CSS budget is unchanged.
- The grid becomes a push relay (FCM/APNs credentials as grid secrets); a self-hosted grid without them still works,
  with ntfy and the foreground service.
- wisp gets one new dependency (`hpke`), the iOS app one (Citadel), Android one optional (Firebase Messaging, `play`
  flavour only).
- ADR-0016's pixel parity no longer covers the apps; `qa/platform.mjs` and the native suites do. The web app at phone
  width keeps v1 parity, so `design/mobile.md` Part B stays its spec.
- Rules out: Tauri on mobile, loading the computer's own page in the WebView (v1), LAN HTTP pairing, feature code in
  Java/Swift, features reading the platform.

## Edge cases

| Case | What happens | Test |
|---|---|---|
| The browser at 390 px | no `data-platform`; v1 parity applies | `packages/shell/test/platform.test.ts` `a_browser_never_sets_data_platform` |
| A feature or plugin UI reads `data-platform` or `platform.kind === 'ios'` | `npm run check` fails | `tools/check/test/platform-branch.test.ts` `a_feature_reading_the_platform_fails` |
| A My Stuff app in the phone app | its frame gets the platform tokens as CSS variables | `packages/ui/test/app-frame.test.ts` `an_app_frame_gets_the_platform_tokens` |
| A host key changed since it was kept | refused, *This computer's key changed*, nothing sent | `mobile/android/.../TunnelTest.java` `aChangedHostKeyIsRefused`; `mobile/ios/App/AppTests/TunnelTests.swift` `testAChangedHostKeyIsRefused` |
| A QR-pinned fingerprint and a different key on the first connect | refused before the password is sent | `TunnelTest` `aPinnedFingerprintRefusesAnotherKeyFirstTime`; same in `TunnelTests` |
| The password after keying | not kept anywhere | `KeysTest` `thePasswordIsForgottenAfterKeying` |
| v1's key files and `localStorage` tokens | moved into the Keystore/Keychain once, then deleted | `mobile/web/test/migrate.test.ts` `v1_tokens_leave_web_storage`; `KeysTest` `v1KeyFilesMoveIntoTheKeystoreOnce` |
| Any token in web storage after a sign-in | never | `mobile/web/test/bridge-contract.test.ts` `tokens_never_land_in_web_storage` |
| Two SSH computers at once | two local ports, two peers, one merged list | `mobile/web/test/peers.test.ts` `two_ssh_computers_get_two_ports` |
| SSH up but wisp stopped there | bootstrap again after two missed health checks (v1) | `TunnelTest` `aStoppedWispIsStartedAgainAfterTwoMissedChecks` |
| A grid box asleep | *Asleep* with *Wake*; the list doesn't wake it | `mobile/web/test/transport.test.ts` `an_asleep_box_is_not_woken_by_the_list` |
| A revoked device token (401) | cached rows stay, read-only, with *Sign in again* | `transport.test.ts` `a_revoked_token_keeps_the_rows_and_asks_to_sign_in` |
| A push payload at the relay | ciphertext, handle and collapse key only | `apps/grid/tests/push.rs` `the_relay_sees_only_ciphertext` |
| A push sealed by wisp | opens with the device's key only | `plugins/push/host/tests/push.rs` `a_notification_opens_with_its_device_key_only` |
| A second push for the same task | replaces the first | `mobile/web/test/notify.test.ts` `a_second_push_for_a_task_replaces_the_first` |
| A push for the task open on screen | no banner; the page already shows it | `notify.test.ts` `a_push_for_the_open_task_shows_no_banner` |
| The phone uninstalled (FCM/APNs: token gone) | the relay drops the handle; wisp turns the channel off with the reason | `apps/grid/tests/push.rs` `a_gone_token_turns_its_channel_off` |
| Answering a question from the notification | the answer lands once, signed, as from the app | `plugins/push/host/tests/push.rs` `a_notification_answer_lands_once` |
| Android without Play services | UnifiedPush if a distributor is there, otherwise the foreground service | `mobile/android/.../PushTest.java` `withoutPlayServicesUnifiedPushIsOffered` |
| Push paired, app in the background | the tunnel and service stop after 5 min | `TunnelServiceTest` `withPushTheServiceStopsInTheBackground` |
| Notifications refused | Settings → Phone says so with *Open settings*; no prompt loop | `packages/shell/test/platform.test.ts` `a_refused_permission_shows_its_row_not_a_prompt` |
| *Mark done* offline | done at once, *Waiting to send*, sent once on reconnect | `packages/client/test/commands.test.ts` `an_offline_command_is_sent_once_on_reconnect` |
| The app killed with commands waiting | they survive the restart and are sent | `commands.test.ts` `pending_commands_survive_a_restart` |
| 4,000 cached tasks, no network | the list paints ≤ 800 ms, scrolls with 0 long tasks | `mobile/web/test/first-paint.test.ts` `four_thousand_cached_tasks_paint_before_the_network`; bench `spikes/phone-bench/ (extended to the app)` and `tools/budgets.ts` `phone.firstList` |
| A stream silent past two keepalives | dropped and reopened from the cursor | `packages/sync/test/client.test.ts` `a_silent_stream_is_reopened_from_its_cursor` |
| Pull to refresh while backing off | reopens now | `mobile/web/test/peers.test.ts` `pull_to_refresh_skips_the_backoff` |
| iOS goes to the background and back | streams closed, then resumed from cursors, no full sync | `transport.test.ts` `a_resumed_peer_asks_from_its_cursor` |
| A wisp too old for the app | *Update wisp on Studio* with the command | `transport.test.ts` `an_older_wisp_says_update` |
| A notification tapped, its computer offline | the cached task opens, marked offline | `notify.test.ts` `a_tap_opens_the_cached_task_offline` |
| 11 files or 25 MB shared in | the first 10 / up to 20 MB, and says what was left | `SharedTest` `overTheCapKeepsTenAndSaysSo`; `ShareTests` `testOverTheCapKeepsTen` |
| Shared files before Send | nothing leaves the phone | `mobile/web/test/share.test.ts` `shared_files_upload_only_on_send` |
| Shared with no computer connected | New task waits with *Pick a computer* | `share.test.ts` `a_share_with_no_computer_waits` |
| Local Network refused (iOS) | *Allow Local Network in Settings*; typed and Tailscale addresses still work | `TunnelTests` `testRefusedLocalNetworkSaysHowToAllowIt` |
| A swipe that starts in a code block or diff | scrolls the block, never swipes the row/page | `packages/ui/test/gestures.test.ts` `a_swipe_in_a_scroller_scrolls_it` |
| A swipe starting within 24 px of an edge | the system's Back | `gestures.test.ts` `an_edge_swipe_is_the_systems` |
| A gesture with no visible twin | won't type-check; `qa/platform.mjs` fails a view missing the twin | `gestures.test.ts` `every_gesture_names_its_control` |
| Text at 200 % (Dynamic Type, Android font scale) | control text never wraps; bars fold by priority | `qa/layout.mjs` (phone views at 200 %) |

## Open questions (for the owner)

1. **Sign in with Apple** on the grid, needed for the iOS app to pass review with GitHub/Google sign-in. Default:
   add it before the iOS app ships.
2. **Push without a wisp Cloud account** (SSH-only users): the relay could register devices with App Attest / Play
   Integrity and no account. Default: require a (free) wisp Cloud sign-in for push; SSH-only keeps ntfy and the
   foreground service.
3. **Where the app opens:** v1 opens on the computers list, never inside one. With several computers in one list, v2
   could open on Tasks (all computers). Default: v1's behaviour (Computers) until you say.
4. **Android dynamic colour** (Material You wallpaper colours): default off, wisp's accent everywhere.
5. **Icons on iOS:** SF Symbols' licence limits them to Apple platforms; the kit is shared. Default: wisp's own icon
   set drawn at SF metrics on iOS, Material Symbols Rounded on Android.
