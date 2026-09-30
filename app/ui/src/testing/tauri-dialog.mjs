// node --test stand-in for '@tauri-apps/plugin-dialog' (see hooks.mjs): no native dialog. Open
// answers nothing; Save as answers `globalThis.__TAURI_TEST_SAVE_PATH__`, or nothing.
export async function open() {
  return null;
}

export async function save() {
  return globalThis.__TAURI_TEST_SAVE_PATH__ ?? null;
}

export async function message() {}
