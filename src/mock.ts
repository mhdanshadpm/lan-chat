// Browser-only mock bridge. It exists so the UI can be developed and reviewed
// in a normal browser (`npm run dev`) without the Tauri runtime. No mock code
// is used inside the desktop app.
import {
  DEFAULT_SETTINGS,
  type Conversation,
  type CoreEvent,
  type FileProgress,
  type Group,
  type Identity,
  type Message,
  type Peer,
  type Settings,
} from './types';

export const isMock =
  typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);

const now = Date.now();
const me = '9b2f7c1e-1a2b-4c3d-8e9f-000000000001';
const maya = '9b2f7c1e-1a2b-4c3d-8e9f-000000000002';
const devon = '9b2f7c1e-1a2b-4c3d-8e9f-000000000003';
const piper = '9b2f7c1e-1a2b-4c3d-8e9f-000000000004';
const everyone = '00000000-0000-0000-0000-000000000001';
const studio = 'a1b2c3d4-0000-4000-8000-000000000010';
const demoFile = 'f1e2d3c4-0000-4000-8000-000000000099';

const identity: Identity = {
  peer_id: me,
  display_name: 'Avery',
  avatar_color: 0x74aaf7,
  device_name: 'Studio MacBook',
  created_at: now - 86400000 * 12,
};

const peers: Peer[] = [
  {
    id: maya,
    display_name: 'Maya Chen',
    avatar_color: 0xe06c75,
    device_name: "Maya's MacBook",
    online: true,
    last_seen: now,
    addr: '192.168.1.24',
    port: 51234,
    secure: true,
  },
  {
    id: devon,
    display_name: 'Devon Park',
    avatar_color: 0x8bd49a,
    device_name: 'DESKTOP-DEVON',
    online: true,
    last_seen: now,
    addr: '192.168.1.31',
    port: 51235,
    secure: true,
  },
  {
    id: piper,
    display_name: 'Piper Ortiz',
    avatar_color: 0x9b8cf5,
    device_name: 'iPad',
    online: false,
    last_seen: now - 1000 * 60 * 42,
    addr: '192.168.1.44',
    port: 51236,
    secure: true,
  },
];

const groups: Group[] = [
  {
    id: everyone,
    name: 'Everyone',
    avatar_color: 0x10b3a2,
    members: [me, maya, devon, piper],
    created_by: me,
    created_at: now - 86400000 * 12,
    is_default: true,
    version: 1,
  },
  {
    id: studio,
    name: 'Studio',
    avatar_color: 0xf29e6e,
    members: [me, maya, devon],
    created_by: maya,
    created_at: now - 86400000 * 3,
    is_default: false,
    version: 4,
  },
];

let conversations: Conversation[] = [
  {
    id: everyone,
    kind: 'group',
    title: 'Everyone',
    avatar_color: 0x10b3a2,
    unread: 1,
    muted: false,
    last_ts: now - 1000 * 60 * 20,
    last_preview: 'The new router is on the shelf if anyone needs it.',
    last_sender: 'Maya Chen',
    mentioned: false,
    online: null,
    member_count: 4,
    is_everyone: true,
  },
  {
    id: studio,
    kind: 'group',
    title: 'Studio',
    avatar_color: 0xf29e6e,
    unread: 1,
    muted: false,
    last_ts: now - 1000 * 60 * 9,
    last_preview: 'Uploaded the revised poster — can you sanity-check the margins?',
    last_sender: 'Devon Park',
    mentioned: true,
    online: null,
    member_count: 3,
    is_everyone: false,
  },
  {
    id: maya,
    kind: 'direct',
    title: 'Maya Chen',
    avatar_color: 0xe06c75,
    unread: 0,
    muted: false,
    last_ts: now - 1000 * 60 * 43,
    last_preview: 'Legend.',
    last_sender: 'Maya Chen',
    mentioned: false,
    online: true,
    member_count: 2,
    is_everyone: false,
  },
  {
    id: devon,
    kind: 'direct',
    title: 'Devon Park',
    avatar_color: 0x8bd49a,
    unread: 0,
    muted: false,
    last_ts: now - 1000 * 60 * 60 * 5,
    last_preview: 'Looks good on my side.',
    last_sender: 'Devon Park',
    mentioned: false,
    online: true,
    member_count: 2,
    is_everyone: false,
  },
];

function msg(partial: Partial<Message> & Pick<Message, 'id' | 'conv' | 'conv_kind' | 'sender_id' | 'sender_name' | 'ts'>): Message {
  return {
    kind: 'text',
    text: null,
    file_id: null,
    reply_to: null,
    mentions: [],
    state: 'read',
    outgoing: false,
    ...partial,
  };
}

let messages: Message[] = [
  msg({
    id: '10000000-0000-4000-8000-000000000001',
    conv: everyone,
    conv_kind: 'group',
    sender_id: piper,
    sender_name: 'Piper Ortiz',
    ts: now - 1000 * 60 * 32,
    text: 'Welcome to the LAN! No internet required out here.',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000002',
    conv: everyone,
    conv_kind: 'group',
    sender_id: maya,
    sender_name: 'Maya Chen',
    ts: now - 1000 * 60 * 20,
    text: 'The new router is on the shelf if anyone needs it.',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000003',
    conv: studio,
    conv_kind: 'group',
    sender_id: devon,
    sender_name: 'Devon Park',
    ts: now - 1000 * 60 * 9,
    text: 'Uploaded the revised poster — can you sanity-check the margins?',
    mentions: [me],
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000004',
    conv: studio,
    conv_kind: 'group',
    sender_id: maya,
    sender_name: 'Maya Chen',
    ts: now - 1000 * 60 * 2 * 60,
    kind: 'image',
    file_id: demoFile,
    text: 'Desk setup from this morning',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000005',
    conv: maya,
    conv_kind: 'direct',
    sender_id: maya,
    sender_name: 'Maya Chen',
    ts: now - 1000 * 60 * 45,
    text: 'Morning! Did the build finish?',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000006',
    conv: maya,
    conv_kind: 'direct',
    sender_id: me,
    sender_name: 'Avery',
    ts: now - 1000 * 60 * 44,
    text: 'Yep — the msi and dmg are both up.',
    outgoing: true,
    state: 'read',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000007',
    conv: maya,
    conv_kind: 'direct',
    sender_id: maya,
    sender_name: 'Maya Chen',
    ts: now - 1000 * 60 * 43,
    text: 'Legend.',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000008',
    conv: devon,
    conv_kind: 'direct',
    sender_id: me,
    sender_name: 'Avery',
    ts: now - 1000 * 60 * 60 * 5 - 60000,
    text: 'Can you see the shared folder?',
    outgoing: true,
    state: 'read',
  }),
  msg({
    id: '10000000-0000-4000-8000-000000000009',
    conv: devon,
    conv_kind: 'direct',
    sender_id: devon,
    sender_name: 'Devon Park',
    ts: now - 1000 * 60 * 60 * 5,
    text: 'Looks good on my side.',
  }),
];

let settings: Settings = structuredClone(DEFAULT_SETTINGS);
let activeConv = everyone;

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();
const channel =
  typeof BroadcastChannel !== 'undefined' ? new BroadcastChannel('lanchat-demo') : null;

function notify(name: string, payload: unknown) {
  handlers.get(name)?.forEach((handler) => handler(payload));
}

function emit(event: CoreEvent) {
  notify('core://event', event);
  if (event.type === 'message' && channel) {
    channel.postMessage({ __lanchat: 'message', message: event.message });
  }
}

function touchConversation(message: Message, incrementUnread: boolean) {
  const conv = conversations.find((c) => c.id === message.conv);
  if (!conv) return;
  conv.last_ts = message.ts;
  conv.last_preview =
    message.text?.trim() ||
    (message.kind === 'image' ? 'Photo' : message.kind === 'video' ? 'Video' : 'File');
  conv.last_sender = message.sender_name;
  if (incrementUnread) {
    conv.unread += 1;
    if (message.mentions.includes(me)) conv.mentioned = true;
  }
  conversations = [...conversations].sort((a, b) => b.last_ts - a.last_ts);
  notify('core://event', { type: 'conversation_updated', conversation: conv } as CoreEvent);
}

function appendIncoming(message: Message) {
  if (messages.some((m) => m.id === message.id)) return;
  messages = [...messages, message];
  touchConversation(message, true);
  notify('core://event', { type: 'message', message } as CoreEvent);
}

function appendOutgoing(message: Message) {
  messages = [...messages, message];
  touchConversation(message, false);
  emit({ type: 'message', message });
}

channel?.addEventListener('message', (event) => {
  const data = event.data as { __lanchat?: string; message?: Message };
  if (data?.__lanchat === 'message' && data.message) {
    appendIncoming({ ...data.message, outgoing: false });
  }
});

function newId(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `mock-${Math.random().toString(16).slice(2)}-${Date.now()}`;
}

const replies = [
  'Got it — thanks!',
  'Nice, that works for me.',
  'Give me ten minutes and I’ll check.',
  'Ha, perfect.',
  'On it.',
];

function scheduleReply(conv: string) {
  const peer = conv === maya ? maya : conv === devon ? devon : maya;
  const peerInfo = peers.find((p) => p.id === peer);
  if (!peerInfo) return;
  window.setTimeout(() => {
    appendIncoming(
      msg({
        id: newId(),
        conv,
        conv_kind: conv === everyone || conv === studio ? 'group' : 'direct',
        sender_id: peer,
        sender_name: peerInfo.display_name,
        ts: Date.now(),
        text: replies[Math.floor(Math.random() * replies.length)],
      }),
    );
  }, 1800 + Math.random() * 900);
}

const demoThumb =
  'data:image/svg+xml,' +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#4f5dff"/><stop offset="0.55" stop-color="#7a5cff"/><stop offset="1" stop-color="#19c2b8"/></linearGradient></defs><rect width="480" height="320" fill="url(#g)"/><circle cx="130" cy="250" r="150" fill="rgba(255,255,255,0.12)"/><circle cx="390" cy="80" r="90" fill="rgba(255,255,255,0.16)"/><text x="36" y="70" fill="rgba(255,255,255,0.9)" font-family="system-ui,sans-serif" font-size="22" font-weight="600">Desk setup</text></svg>`,
  );

export async function mockInvoke(
  command: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  await new Promise((resolve) => window.setTimeout(resolve, 18));
  switch (command) {
    case 'bootstrap':
      return {
        identity,
        settings,
        peers,
        conversations,
        groups,
        unread_total: conversations.reduce((sum, c) => sum + c.unread, 0),
        port: 52114,
        platform: 'browser',
        capabilities: {
          bubble_over_fullscreen: false,
          no_focus_bubble: false,
          tray: 'system_tray',
          native_sound: false,
          global_hotkey: false,
          autostart: false,
          dock_icon_toggle: false,
        },
        configured: true,
      };
    case 'save_identity': {
      identity.display_name = String(args?.displayName ?? identity.display_name);
      identity.avatar_color = Number(args?.avatarColor ?? identity.avatar_color);
      return identity;
    }
    case 'list_peers':
      return peers;
    case 'list_conversations':
      return conversations;
    case 'list_groups':
      return groups;
    case 'messages': {
      const conv = String(args?.conv);
      const before = args?.beforeTs as number | null | undefined;
      const limit = Number(args?.limit ?? 80);
      const list = messages
        .filter((m) => m.conv === conv && (!before || m.ts < before))
        .sort((a, b) => a.ts - b.ts);
      return list.slice(-limit);
    }
    case 'search_messages': {
      const query = String(args?.query ?? '').toLowerCase();
      return messages
        .filter((m) => (m.text ?? '').toLowerCase().includes(query))
        .sort((a, b) => b.ts - a.ts);
    }
    case 'send_text': {
      const conv = String(args?.conv);
      activeConv = conv;
      const kind =
        conv === everyone || conv === studio ? ('group' as const) : ('direct' as const);
      const message = msg({
        id: newId(),
        conv,
        conv_kind: kind,
        sender_id: me,
        sender_name: identity.display_name,
        ts: Date.now(),
        text: String(args?.text ?? ''),
        reply_to: (args?.replyTo as string | null) ?? null,
        outgoing: true,
        state: 'sent',
      });
      appendOutgoing(message);
      scheduleReply(conv);
      return message;
    }
    case 'send_file_bytes': {
      const conv = String(args?.conv);
      const mime = String(args?.mime ?? 'application/octet-stream');
      const name = String(args?.name ?? 'file');
      const kind = mime.startsWith('image/')
        ? 'image'
        : mime.startsWith('video/')
          ? 'video'
          : mime.startsWith('audio/')
            ? 'audio'
            : 'file';
      const fileId = newId();
      const message = msg({
        id: newId(),
        conv,
        conv_kind: conv === everyone || conv === studio ? 'group' : 'direct',
        sender_id: me,
        sender_name: identity.display_name,
        ts: Date.now(),
        kind,
        file_id: fileId,
        text: (args?.caption as string | null) ?? null,
        outgoing: true,
        state: 'sending',
      });
      appendOutgoing(message);
      const progress: FileProgress = {
        transfer_id: fileId,
        conv,
        direction: 'outgoing',
        name,
        mime,
        size: 2_400_000,
        received: 0,
        state: 'uploading',
        path: null,
        thumb_path: null,
        peer_id: null,
      };
      emit({ type: 'transfer', file: progress });
      window.setTimeout(() => {
        emit({ type: 'transfer', file: { ...progress, received: progress.size, state: 'done' } });
        notify('core://event', {
          type: 'message_state',
          conv,
          ids: [message.id],
          state: 'sent',
        } as CoreEvent);
      }, 900);
      return message;
    }
    case 'mark_read': {
      const conv = conversations.find((c) => c.id === String(args?.conv));
      if (conv) {
        conv.unread = 0;
        conv.mentioned = false;
        notify('core://event', { type: 'conversation_updated', conversation: conv } as CoreEvent);
      }
      return null;
    }
    case 'set_typing':
    case 'respond_file':
    case 'reveal_file':
    case 'open_file':
    case 'save_bubble_position':
    case 'hide_bubble':
    case 'open_main':
      return null;
    case 'set_muted': {
      const conv = conversations.find((c) => c.id === String(args?.conv));
      if (conv) {
        conv.muted = Boolean(args?.muted);
        notify('core://event', { type: 'conversation_updated', conversation: conv } as CoreEvent);
      }
      return null;
    }
    case 'create_group': {
      const group: Group = {
        id: newId(),
        name: String(args?.name ?? 'New group'),
        avatar_color: 0xf5d06b,
        members: [...((args?.members as string[]) ?? []), me],
        created_by: me,
        created_at: Date.now(),
        is_default: false,
        version: 1,
      };
      groups.push(group);
      conversations.unshift({
        id: group.id,
        kind: 'group',
        title: group.name,
        avatar_color: group.avatar_color,
        unread: 0,
        muted: false,
        last_ts: Date.now(),
        last_preview: 'Group created',
        last_sender: identity.display_name,
        mentioned: false,
        online: null,
        member_count: group.members.length,
        is_everyone: false,
      });
      return group;
    }
    case 'add_group_member': {
      const group = groups.find((g) => g.id === String(args?.groupId));
      const peer = String(args?.peer);
      if (group && !group.members.includes(peer)) group.members.push(peer);
      return null;
    }
    case 'remove_group_member': {
      const group = groups.find((g) => g.id === String(args?.groupId));
      const peer = String(args?.peer);
      if (group) group.members = group.members.filter((m) => m !== peer);
      return null;
    }
    case 'rename_group': {
      const group = groups.find((g) => g.id === String(args?.groupId));
      if (group) group.name = String(args?.name ?? group.name);
      const conv = conversations.find((c) => c.id === group?.id);
      if (conv && group) conv.title = group.name;
      return null;
    }
    case 'leave_group': {
      const id = String(args?.groupId);
      conversations = conversations.filter((c) => c.id !== id);
      return null;
    }
    case 'update_settings': {
      settings = args?.settings as Settings;
      notify('core://event', { type: 'settings', settings } as CoreEvent);
      return settings;
    }
    case 'reset_bubble_position':
    case 'show_bubble':
      return null;
    case 'last_active_conversation':
      return activeConv;
    case 'pending_files':
      return [];
    case 'file_record':
      return null;
    case 'file_thumbnail':
      return demoThumb;
    case 'unread_total':
      return conversations.reduce((sum, c) => sum + c.unread, 0);
    default:
      return null;
  }
}

export function mockOn(name: string, handler: Handler): () => void {
  let set = handlers.get(name);
  if (!set) {
    set = new Set();
    handlers.set(name, set);
  }
  set.add(handler);
  if (name === 'bubble://incoming') {
    window.setTimeout(() => {
      const latest = [...messages].sort((a, b) => b.ts - a.ts)[0];
      if (latest) handler(latest);
    }, 350);
  }
  if (name === 'bubble://summon' || name === 'bubble://open') {
    window.setTimeout(() => handler(activeConv), 120);
  }
  return () => {
    set?.delete(handler);
  };
}
