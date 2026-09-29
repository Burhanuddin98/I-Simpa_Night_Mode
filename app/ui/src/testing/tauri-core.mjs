// node --test stand-in for '@tauri-apps/api/core' (see hooks.mjs): `invoke` goes to the suite's
// backend, `globalThis.__TAURI_TEST_IPC__(cmd, args)`, and a `Channel` only holds `onmessage`,
// which the suite's backend calls with the batches it streams.
export async function invoke(cmd, args) {
  const ipc = globalThis.__TAURI_TEST_IPC__;
  if (typeof ipc !== 'function') throw { code: 'NO_TEST_IPC', message: `no test backend for ${cmd}` };
  return ipc(cmd, args ?? {});
}

export class Channel {
  constructor(onmessage) {
    this.onmessage = onmessage ?? (() => {});
  }
}
