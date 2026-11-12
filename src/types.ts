export type ConvKind = 'direct' | 'group';
export type MessageKind = 'text' | 'image' | 'video' | 'audio' | 'file' | 'system';
export type DeliveryState = 'queued' | 'sending' | 'sent' | 'delivered' | 'read' | 'failed';
export type FileDirection = 'incoming' | 'outgoing';
export type FileState =
  | 'pending_offer'
  | 'awaiting_accept'
  | 'downloading'
  | 'uploading'
  | 'paused'
  | 'done'
  | 'rejected'
  | 'failed';
export type Theme = 'system' | 'light' | 'dark';
export type Corner = 'top_left' | 'top_right' | 'bottom_right' | 'bottom_left';

export interface Identity {
  peer_id: string;
  display_name: string;
  avatar_color: number;
  device_name: string;
  created_at: number;
}

export interface Peer {
  id: string;
  display_name: string;
  avatar_color: number;
  device_name: string;
  online: boolean;
  last_seen: number;
  addr: string | null;
  port: number | null;
  secure: boolean;
}

export interface Message {
  id: string;
  conv: string;
  conv_kind: ConvKind;
  sender_id: string;
  sender_name: string;
  kind: MessageKind;
  text: string | null;
  file_id: string | null;
  reply_to: string | null;
  mentions: string[];
  ts: number;
  state: DeliveryState;
  outgoing: boolean;
}

export interface Conversation {
  id: string;
  kind: ConvKind;
  title: string;
  avatar_color: number;
  unread: number;
  muted: boolean;
  last_ts: number;
  last_preview: string;
  last_sender: string;
  mentioned: boolean;
  online: boolean | null;
  member_count: number;
  is_everyone: boolean;
}

export interface Group {
  id: string;
  name: string;
  avatar_color: number;
  members: string[];
  created_by: string;
  created_at: number;
  is_default: boolean;
  version: number;
}

export interface BubbleSettings {
  enabled: boolean;
  auto_hide_secs: number;
  corner: Corner;
  opacity: number;
  collapsed: boolean;
  x: number | null;
  y: number | null;
  show_on_every_message: boolean;
}

export interface Settings {
  theme: Theme;
  bubble: BubbleSettings;
  passphrase: string | null;
  dnd: boolean;
  notification_sound: boolean;
  launch_at_login: boolean;
  auto_accept_files_mb: number;
  auto_download_files_mb: number;
  max_file_mb: number;
  discovery_port: number;
  read_receipts: boolean;
  typing_indicators: boolean;
  pause_bubble_when_app_focused: boolean;
}

export interface Capabilities {
  bubble_over_fullscreen: boolean;
  no_focus_bubble: boolean;
  tray: 'system_tray' | 'menu_bar';
  native_sound: boolean;
  global_hotkey: boolean;
  autostart: boolean;
  dock_icon_toggle: boolean;
}

export interface Bootstrap {
  identity: Identity;
  settings: Settings;
  peers: Peer[];
  conversations: Conversation[];
  groups: Group[];
  unread_total: number;
  port: number;
  platform: string;
  capabilities: Capabilities;
  configured: boolean;
}

export interface FileRecord {
  id: string;
  message_id: string | null;
  conv: string;
  peer_id: string | null;
  direction: FileDirection;
  name: string;
  path: string | null;
  thumb_path: string | null;
  mime: string;
  size: number;
  received: number;
  sha256: string;
  state: FileState;
  next_index: number;
  chunk_size: number;
  chunks: number;
  created_at: number;
}

export interface FileProgress {
  transfer_id: string;
  conv: string;
  direction: FileDirection;
  name: string;
  mime: string;
  size: number;
  received: number;
  state: FileState;
  path: string | null;
  thumb_path: string | null;
  peer_id: string | null;
}

export type CoreEvent =
  | { type: 'peer'; peer: Peer }
  | { type: 'peer_offline'; id: string }
  | { type: 'message'; message: Message }
  | { type: 'message_state'; conv: string; ids: string[]; state: DeliveryState }
  | { type: 'typing'; peer: string; peer_name: string; conv: string; active: boolean }
  | { type: 'group_updated'; group: Group }
  | { type: 'conversation_updated'; conversation: Conversation }
  | { type: 'transfer'; file: FileProgress }
  | { type: 'error'; context: string; message: string }
  | { type: 'network_changed' }
  | { type: 'listening'; port: number }
  | { type: 'settings'; settings: Settings };

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  bubble: {
    enabled: true,
    auto_hide_secs: 8,
    corner: 'bottom_right',
    opacity: 0.96,
    collapsed: false,
    x: null,
    y: null,
    show_on_every_message: true,
  },
  passphrase: null,
  dnd: false,
  notification_sound: true,
  launch_at_login: false,
  auto_accept_files_mb: 10,
  auto_download_files_mb: 25,
  max_file_mb: 2048,
  discovery_port: 47823,
  read_receipts: true,
  typing_indicators: true,
  pause_bubble_when_app_focused: true,
};
