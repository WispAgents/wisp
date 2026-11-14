// @wisp/mobile: the Android app's side of the bundle (plan §7.3). The native bridge (sshj tunnels,
// wisp Grid links, Back, notifications), saved computers as @wisp/client transports and peers, and
// bootPhone, which apps/web's entry calls inside the app.

export { capacitorBridge, exposePlugins, hostKeyRefused, PLUGIN } from './bridge.ts';
export type { CapacitorLike, Computer, ComputerPatch, GridComputer, NativeBridge, NewComputer, Reached, SshComputer, TunnelEvent, TunnelState } from './bridge.ts';
export { bridgeTransport, gridAsleep } from './transport.ts';
export type { BridgeTransportOptions } from './transport.ts';
export { followComputers, peerConfig, peerOf, phonePeers, PHONE } from './peers.ts';
export type { PhonePeersOptions } from './peers.ts';
export { fromV1, migrateV1 } from './migrate.ts';
export { fromPairingCode } from './pair.ts';
export type { StorageLike } from './migrate.ts';
export { back, bootPhone, onPhone, phonePlatform } from './boot.ts';
export type { Phone, PhoneOptions, PhoneWindow } from './boot.ts';
