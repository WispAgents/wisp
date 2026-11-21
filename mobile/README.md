# wisp for Android (prototype)

wisp on your phone, for your own computers. Sign in to wisp Cloud once and every computer you
added to it (from Machines on that computer) is listed; or add one over SSH (its address and your
password). Tap it and you're in that computer's wisp: start tasks, answer your agents, read the reports. The
agents keep running there when the phone sleeps. **Computers** in wisp's top bar brings you back
to the list, to switch to another one or add one.

*Computers* is the app's word for what it connects to; *Machines* stays the desktop feature that
reaches other computers over SSH (it isn't shown in the app).

The page is the phone app, `web/` (`@wisp/mobile`, ADR-0065): wisp's own shell and every plugin's UI, with the
Computers destination, reaching each saved computer through the native bridge (`WispSshPlugin`: sshj tunnels, wisp
Grid) over the sync protocol. `npm run web` builds it with vite into `www/` (Capacitor's `webDir`, gitignored), and
`cap sync` copies that into the APK's `assets/public/`. wisp starts on an SSH computer with the desktop's own script:
`app/build.gradle` copies `plugins/machines/core/bootstrap.sh` into the APK's assets with the wisp version filled in
(`Bootstrap.java`).

```
web/                 the phone app (@wisp/mobile): npm run dev there for a browser preview with mock computers
www/                 its build (npm run web), what Capacitor ships: never edited by hand
android/app/src/main/java/dev/trywisp/mobile/
  Tunnel.java        SSH (sshj), runs the script, forwards the port, reconnects (plain Java)
  Keys.java          the phone's keys: Ed25519 made here, or imported OpenSSH/PEM/PuTTY
  WispSshPlugin.java the Capacitor plugin the page calls
  TunnelService.java keeps the app running while connected (foreground service)
  MainActivity.java  window.wispAndroid: notifications for wisp's page; Alerts.java posts them
```

## wisp Cloud

No computer to keep on? The first screen is **Continue with GitHub** / **Continue with Google**
on `grid.trywisp.dev`: the app asks the grid for a device code and opens the browser at
`/auth/<provider>/start`, returning to `/activate?code=…&app=android`, so you sign in and Allow this
phone in one go. Your cloud computer joins the list; tap it and you're in its wisp: no SSH, no
tunnel. *Use your own wisp Grid* takes another grid's address and shows the code to allow there.

Your own computers come along: each one you added to the grid (wisp on it: Machines → *Reach this
computer from anywhere*) is listed under the cloud one, marked *Your computer · via grid*, with
**Open** while it's online. The list comes from `GET /api/computers` with the device token (the
last one is kept, so the screen paints at once), and Open asks for a one-time link to that
computer (`/api/device/link` with `computer`). It holds a tunnel open to the grid, so this
reaches it from anywhere with no SSH. Over SSH (below) is folded under *Connect over SSH instead*. The app keeps a device token for it and asks the grid for a
one-time link each time it opens; `Grid.java` lets the grid's pages load in the app, and the page's
service worker leaves the grid's own `/_grid/…` links alone. Sign in to Claude Code, Codex or
GitHub there in Settings → Agents. See [grid/README.md](../grid/README.md).

## Use it over SSH

1. **Get a computer ready.** Turn on SSH there: on a Mac, System Settings → General → Sharing →
   *Remote Login*; on Linux, `openssh-server`. That's all it needs.
2. **Add it in the app:** its address (`you@its-name`, or `you@its-name:2222`) and your password
   on it. The password is used once: the app signs in with it, adds the phone's own key there
   (`@@wisp keyed`) and forgets it. Prefer a key? *Use a key instead* shows the
   `authorized_keys` command, or import one under *Keys on this phone*. With Tailscale SSH on a
   Linux computer (`sudo tailscale set --ssh`) nothing is needed: the app tries that first.
3. **Connect.** The card shows the steps (SSH, Node.js, wisp, tunnel); a computer without Node.js
   gets it in `~/.wisp/node`, and wisp comes from `npx`. Then wisp opens. Pick a coding agent there
   (Settings → Agents: opencode installs in one click).
4. **Switch:** the wisp logo or **Computers** in wisp's top bar (or Settings → Computers) goes back to the list.
   The app opens the last computer used each time it starts.

Away from home, put the
computers and the phone on [Tailscale](https://tailscale.com) and use the names it gives. Step by
step: https://trywisp.dev/guides/connect-machines-with-tailscale.html

**When wisp stops there.** SSH can stay up after wisp itself stopped on the computer (quit,
crashed). While connected, `Tunnel` asks wisp's `/api/health` every 15s. After two misses, if sshd
also refuses a channel to wisp's port, it runs the bootstrap script again, which starts wisp. A
page that still fails to load sends the WebView back to the list (`MainActivity`, `?reopen`),
which reopens wisp once the tunnel says `open`, so Chrome's error page never shows.

**Notifications.** When a task finishes, asks or needs approval while the app is off screen,
you get an Android notification; tapping it opens that task. The WebView has no Notification
API, so `MainActivity` gives wisp's page a `wispAndroid` bridge (`web/js/features/notify.js`
uses it). While a machine is connected, `TunnelService` (a foreground service with a quiet
*Connected to …* notification) keeps the app, the tunnel and the page running in the background.
A cloud computer has no tunnel but needs the same: the list starts it before opening one
(`stayOpen`) and stops it when it's back with nothing connected.

## Build

Needs Node 20+, a JDK 21 (Android Studio's bundled one works) and the Android SDK (platform 36).

```sh
cd mobile
npm install          # here, and once at the repo root (the workspaces web/ builds from)
npm run build        # vite build of web/ → www → cap sync → ./gradlew assembleDebug
# → android/app/build/outputs/apk/debug/app-debug.apk
npm run open         # or open it in Android Studio
cd web && npm run dev   # the app in a desktop browser with mock computers: http://127.0.0.1:5190/?platform=android
```

Without `JAVA_HOME` set: `export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"`.

## Google Play

The launcher icon is wisp's logo (`docs/assets/icon.svg`) as an adaptive icon: vector layers in
`res/drawable/ic_launcher_{background,foreground,monochrome}.xml` (the last one is Android 13's
themed icon). The store art is in `store/`: `icon.png` (512×512) and `feature-graphic.png`
(1024×500), drawn from the `.svg` next to each. `node scripts/logo.mjs` writes all of these vectors.

1. **Make an upload key**, once, and keep it out of git (backed up: losing it means asking Google
   to reset it):
   ```sh
   keytool -genkeypair -keystore ~/wisp-upload.jks -alias upload -keyalg RSA -keysize 2048 -validity 10000
   ```
   and write `android/keystore.properties` (gitignored):
   ```
   storeFile=/Users/you/wisp-upload.jks
   storePassword=…
   keyAlias=upload
   keyPassword=…
   ```
2. **Build and publish in CI:** `.github/workflows/android.yml` builds a signed
   `wisp-android-<versionName>-<versionCode>.aab` on each push to main that touches the app (or
   `gh workflow run android`) and keeps it as the run's artifact. It signs with the
   `ANDROID_UPLOAD_KEYSTORE` (the `.jks`, base64) and `ANDROID_UPLOAD_KEY_PASSWORD` secrets;
   versionCode is 100 + the run number and versionName is `package.json`'s version.
   With a `PLAY_SERVICE_ACCOUNT_JSON` secret it then publishes (`scripts/play.mjs`), as npm
   releases do: every build goes to *Internal testing*, and the build that bumps `package.json`'s
   version also goes to *Production* (or the track in the `PLAY_PROMOTE_TRACK` repo variable, e.g.
   `alpha` for closed testing while production is locked). A prerelease version (`1.0.0-alpha.1`:
   one with a `-`) is never promoted: it stays on *Internal testing*. *What's new* is `whatsnew.txt` when it
   has text, else the CHANGELOG headlines about the phone app (`node scripts/play.mjs notes`).
   The key, once: in Google Cloud turn on the *Google Play Android Developer API*, make a service
   account and a JSON key for it, then in Play Console → *Users and permissions* invite its email
   with *Release to testing tracks* and *Release to production* on wisp;
   `gh secret set PLAY_SERVICE_ACCOUNT_JSON < key.json`.
3. **Or build it here:** `npm run sync && cd android && ./gradlew bundleRelease -PversionCode=<n>`
   (higher than the last upload; it defaults to 1)
   → `android/app/build/outputs/bundle/release/app-release.aab`.
4. **In the Play Console** (a developer account is a one-time $25): *Create app*, name `wisp`,
   app, free. Then *Release → Testing → Internal testing → Create release*, opt in to Play App
   Signing and upload the `.aab`. Fill the store listing with `store/icon.png`,
   `store/feature-graphic.png` and 2+ phone screenshots.
5. **App content** asks for:
   - *Privacy policy:* https://trywisp.dev/privacy.html (`docs/privacy.md`).
   - *Data safety:* the app's own code collects nothing. The wisp page it shows sends
     anonymous usage stats (app info and performance, crash logs; optional, Settings → Privacy)
     and, when you send it, feedback. wisp Cloud sign-in collects email, name and user IDs for
     account management. Everything is encrypted in transit.
   - *Account deletion:* https://trywisp.dev/privacy.html#delete-your-wisp-cloud-account.
   - *Foreground service:* `specialUse` keeps the user's SSH tunnel open. Play wants a short
     video of the *Connected to …* notification.
   - *Target audience:* not for children. *App access:* reviewers need a way in, such as a wisp
     Cloud test account.
6. **Go public:** a new personal developer account has to run a closed test with 12+ testers for
   14 days before *Production* unlocks; organisation accounts can promote straight from internal
   testing.

## Test

```sh
cd mobile/android && ./gradlew testDebugUnitTest
```

`KeysTest` always runs: it makes, lists, deletes and imports keys, checks fingerprints against
`ssh-keygen -lf`, moves v1's key files into the vault once and checks the password is dropped once
the phone's key is added. `TunnelTest`'s ADR-0065 cases (a pinned fingerprint, a changed host key,
wisp stopped there, two computers at once) run against an sshd in the test's JVM (`FakeSshd`,
Apache MINA sshd, test-only). `ComputersTest` replays `web/test/fixtures/android-bridge.json`, the
recording `mobile/web`'s bridge-contract suite replays too. Three older `TunnelTest` cases need a
real sshd and wisp, so they run only when asked:

```sh
# a throwaway sshd on 127.0.0.1:2299 that accepts /tmp/wsshd/client; the script starts this
# checkout's wisp on 4733 (and starts it again when startsWispAgainWhenItStops kills it)
node -e "import('../../server/machines/bootstrap.js').then(m => process.stdout.write(m.bootstrapScript({ port: 4733, command: 'node ' + require('path').resolve('../../bin/wisp.js'), apiVersion: 'any' })))" > /tmp/wsshd/script.sh
WISP_SSH_TEST=$USER@127.0.0.1:2299 WISP_SSH_KEY=/tmp/wsshd/client \
WISP_SSH_SCRIPT=/tmp/wsshd/script.sh WISP_SSH_PORT=4733 ./gradlew testDebugUnitTest
```

That test connects, runs the script, opens the tunnel and loads wisp's page through it. It also
drops the connection and checks that it comes back, and checks that an unknown key and a
changed host key both fail with a clear message.

## Not yet

- **Closed app, deep sleep.** Notifications come from wisp's page, so they stop when the app is
  swiped away (that also disconnects) or Android kills it, and deep Doze can delay them. Use
  wisp's Phone settings (ntfy / Telegram) for alerts that don't depend on the app.
- **Bonjour/mDNS** addresses (ADR-0065 §2.2) aren't offered yet: type the address or scan the QR.

## Keys, tokens and host keys (v2, ADR-0065 §2)

- **Keys and device tokens** are sealed with an AES-256-GCM key made in the Android Keystore
  (`KeystoreVault`, `SealedVault`): the private `wisp.vault` preferences hold only ciphertext. Ed25519
  and imported keys can't live in the Keystore itself on Android 8–12, so each is sealed with that key.
  The first v2 start moves v1's key files (`files/keys/`) in and deletes them (`Keys.moveV1`); the
  bundle moves v1's `localStorage` list in and deletes it (`mobile/web/src/migrate.ts`).
- **Tokens never reach the page's storage:** `Computers` keeps a grid computer's token in the vault,
  `list()` never answers it, and only `connect`'s answer (`Reached`) carries it, per connect.
- **The password** signs in once, adds the phone's key (`@@wisp keyed`) and is dropped.
- **Host keys** are pinned when scanned from the desktop's QR code
  (`wisp+ssh://you@host:port?fp=SHA256:…&name=Studio`, `mobile/web/src/pair.ts`): another key is refused
  in the key exchange, before anything is sent. Typed by hand, the first key is kept. A changed key is
  refused with *This computer's key changed: it is now SHA256:…*; `trustHostKey({id, fingerprint})`
  keeps the new one only if it is the one shown (the page asks first).
- **Several computers at once:** each tunnel takes wisp's port on the phone when free, else any free
  port, and `connect` says which.
- **The foreground service** runs only while an SSH computer without push is connected and the
  *Stay connected* switch (`setStayConnected`, off by default) is on (`Stay`).
- **iOS**: the same `www/` and plugin API, with an SSH library (Citadel) behind it.
