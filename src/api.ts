import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isMock, mockInvoke, mockOn } from './mock';
import type {
  Bootstrap,
  Conversation,
  CoreEvent,
  FileRecord,
  Group,
  Identity,
  Message,
  Peer,
  Settings,
} from './types';

export const isTauri = !isMock;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isMock) return mockInvoke(cmd, args) as Promise<T>;
  return invoke<T>(cmd, args);
}

export const api = {
  bootstrap: () => call<Bootstrap>('bootstrap'),
  saveIdentity: (displayName: string, avatarColor: number) =>
    call<Identity>('save_identity', { displayName, avatarColor }),
  listPeers: () => call<Peer[]>('list_peers'),
  listConversations: () => call<Conversation[]>('list_conversations'),
  listGroups: () => call<Group[]>('list_groups'),
  messages: (conv: string, beforeTs?: number | null, limit = 80) =>
    call<Message[]>('messages', { conv, beforeTs, limit }),
  searchMessages: (query: string, limit = 100) =>
    call<Message[]>('search_messages', { query, limit }),
  sendText: (conv: string, text: string, replyTo?: string | null) =>
    call<Message>('send_text', { conv, text, replyTo }),
  sendFile: (conv: string, path: string, caption?: string | null) =>
    call<Message>('send_file', { conv, path, caption }),
  sendFileBytes: (
    conv: string,
    name: string,
    mime: string,
    dataB64: string,
    caption?: string | null,
  ) => call<Message>('send_file_bytes', { conv, name, mime, dataB64, caption }),
  markRead: (conv: string) => call<void>('mark_read', { conv }),
  setTyping: (conv: string, active: boolean) => call<void>('set_typing', { conv, active }),
  setMuted: (conv: string, muted: boolean) => call<void>('set_muted', { conv, muted }),
  createGroup: (name: string, members: string[]) =>
    call<Group>('create_group', { name, members }),
  addGroupMember: (groupId: string, peer: string) =>
    call<void>('add_group_member', { groupId, peer }),
  removeGroupMember: (groupId: string, peer: string) =>
    call<void>('remove_group_member', { groupId, peer }),
  renameGroup: (groupId: string, name: string) =>
    call<void>('rename_group', { groupId, name }),
  leaveGroup: (groupId: string) => call<void>('leave_group', { groupId }),
  updateSettings: (settings: Settings) => call<Settings>('update_settings', { settings }),
  saveBubblePosition: (x: number, y: number) =>
    call<void>('save_bubble_position', { x, y }),
  resetBubblePosition: () => call<void>('reset_bubble_position'),
  showBubble: (conv?: string | null) => call<void>('show_bubble', { conv }),
  hideBubble: () => call<void>('hide_bubble'),
  openMain: (conv?: string | null) => call<void>('open_main', { conv }),
  lastActiveConversation: () => call<string | null>('last_active_conversation'),
  pendingFiles: () => call<FileRecord[]>('pending_files'),
  fileRecord: (transferId: string) => call<FileRecord | null>('file_record', { transferId }),
  respondFile: (transferId: string, accept: boolean) =>
    call<void>('respond_file', { transferId, accept }),
  fileThumbnail: (transferId: string) =>
    call<string | null>('file_thumbnail', { transferId }),
  revealFile: (transferId: string) => call<void>('reveal_file', { transferId }),
  openFile: (transferId: string) => call<void>('open_file', { transferId }),
  unreadTotal: () => call<number>('unread_total'),
};

export async function onCoreEvent(handler: (event: CoreEvent) => void): Promise<() => void> {
  if (isMock) return mockOn('core://event', handler as (payload: unknown) => void);
  return listen<CoreEvent>('core://event', (event) => handler(event.payload));
}

export async function onWindowEvent<T>(
  name: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (isMock) return mockOn(name, handler as (payload: unknown) => void);
  return listen<T>(name, (event) => handler(event.payload));
}

export async function onMoved(
  handler: (position: { x: number; y: number }) => void,
): Promise<() => void> {
  if (isMock) return () => undefined;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow().onMoved((event) => handler(event.payload));
}

export async function currentOuterPosition(): Promise<{ x: number; y: number } | null> {
  if (isMock) return null;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return await getCurrentWindow().outerPosition();
  } catch {
    return null;
  }
}

export async function onDragDropFiles(
  handler: (paths: string[]) => void,
): Promise<() => void> {
  if (isMock) return () => undefined;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow().onDragDropEvent((event) => {
    if (event.payload.type === 'drop') handler(event.payload.paths);
  });
}

export async function startWindowDrag(): Promise<void> {
  if (isMock) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().startDragging();
}

export async function ensureNotificationPermission(): Promise<boolean> {
  if (isMock) return false;
  try {
    const plugin = await import('@tauri-apps/plugin-notification');
    if (await plugin.isPermissionGranted()) return true;
    return (await plugin.requestPermission()) === 'granted';
  } catch {
    return false;
  }
}

export function isTauriWindow(): boolean {
  return isTauri;
}
