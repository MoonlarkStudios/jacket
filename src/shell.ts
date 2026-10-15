import { invoke } from '@tauri-apps/api/core';

export type MenuRequest =
  | { kind: 'new' }
  | { kind: 'settings' }
  | { kind: 'authPrototype' }
  | { kind: 'example' };

export function takeMenuRequest() {
  return invoke<MenuRequest | null>('take_menu_request');
}
