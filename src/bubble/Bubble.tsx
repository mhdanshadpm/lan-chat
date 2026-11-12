import { useCallback, useEffect, useRef, useState } from 'react';
import {
  api,
  onCoreEvent,
  onDragDropFiles,
  onMoved,
  onWindowEvent,
  startWindowDrag,
} from '../api';
import type { Conversation, Message, Settings } from '../types';
import { Avatar } from '../components/Avatar';
import {
  IconCheck,
  IconChevronLeft,
  IconChevronRight,
  IconExternal,
  IconFile,
  IconImage,
  IconMic,
  IconMinimize,
  IconPaperclip,
  IconSend,
  IconSmile,
  IconVideo,
  IconX,
} from '../components/Icons';
import { cls, firstName, readAsBase64 } from '../utils';

const EMOJIS = ['👍', '❤️', '😂', '🎉', '🔥', '✅', '🙏', '👀', '😊', '😅', '🤝', '🚀'];

interface BubbleItem {
  conv: string;
  title: string;
  kind: Conversation['kind'];
  color: number;
  isEveryone: boolean;
  message: Message | null;
  fallback: string;
  unread: number;
}

function hashColor(id: string): number {
  let hash = 0;
  for (let i = 0; i < id.length; i += 1) hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  const palette = [0xe06c75, 0xf29e6e, 0xf5d06b, 0x8bd49a, 0x6bd3c9, 0x74aaf7, 0x9b8cf5, 0xe08cf5];
  return palette[hash % palette.length];
}

export function Bubble() {
  const [ready, setReady] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [stack, setStack] = useState<BubbleItem[]>([]);
  const [index, setIndex] = useState(0);
  const [collapsed, setCollapsed] = useState(false);
  const [draft, setDraft] = useState('');
  const [sent, setSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [emojiOpen, setEmojiOpen] = useState(false);
  const [tick, setTick] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const convsRef = useRef<Conversation[]>([]);
  const settingsRef = useRef<Settings | null>(null);
  const stackRef = useRef<BubbleItem[]>([]);
  const indexRef = useRef(0);
  const sentTimer = useRef<number | undefined>(undefined);
  const errorTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    convsRef.current = conversations;
  }, [conversations]);
  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);
  useEffect(() => {
    stackRef.current = stack;
  }, [stack]);
  useEffect(() => {
    indexRef.current = index;
  }, [index]);

  useEffect(() => {
    if (settings) setCollapsed(settings.bubble.collapsed);
  }, [settings?.bubble.collapsed]);

  const raiseError = useCallback((message: string) => {
    setError(message);
    if (errorTimer.current) window.clearTimeout(errorTimer.current);
    errorTimer.current = window.setTimeout(() => setError(null), 5000);
  }, []);

  const flashSent = useCallback(() => {
    setSent(true);
    if (sentTimer.current) window.clearTimeout(sentTimer.current);
    sentTimer.current = window.setTimeout(() => setSent(false), 1800);
  }, []);

  useEffect(() => {
    let alive = true;
    api
      .bootstrap()
      .then((data) => {
        if (!alive) return;
        setSettings(data.settings);
        setConversations([...data.conversations].sort((a, b) => b.last_ts - a.last_ts));
        setCollapsed(data.settings.bubble.collapsed);
        setReady(true);
      })
      .catch((err) => raiseError(String(err)));
    return () => {
      alive = false;
    };
  }, [raiseError]);

  useEffect(() => {
    let disposed = false;
    const disposers: Array<() => void> = [];
    const keep = (promise: Promise<() => void>) => {
      promise
        .then((dispose) => {
          if (disposed) dispose();
          else disposers.push(dispose);
        })
        .catch(() => undefined);
    };

    const handleIncoming = (message: Message) => {
      setStack((prev) => {
        const existing = prev.find((item) => item.conv === message.conv);
        const conv = convsRef.current.find((item) => item.id === message.conv);
        const base: BubbleItem = existing ?? {
          conv: message.conv,
          title: conv?.title ?? message.sender_name,
          kind: conv?.kind ?? message.conv_kind,
          color: conv?.avatar_color ?? hashColor(message.sender_id),
          isEveryone: conv?.is_everyone ?? false,
          message: null,
          fallback: '',
          unread: 0,
        };
        const rest = prev.filter((item) => item.conv !== message.conv);
        return [
          { ...base, message, unread: base.unread + (message.outgoing ? 0 : 1) },
          ...rest,
        ].slice(0, 6);
      });
      setIndex(0);
      setSent(false);
    };

    const openFor = (convId: string | null) => {
      setCollapsed(false);
      const convs = convsRef.current;
      const id = convId ?? stackRef.current[0]?.conv ?? convs[0]?.id ?? null;
      if (id) {
        const conv = convs.find((item) => item.id === id);
        setStack((prev) => {
          const existing = prev.find((item) => item.conv === id);
          if (existing) return [existing, ...prev.filter((item) => item.conv !== id)];
          if (!conv) return prev;
          return [
            {
              conv: conv.id,
              title: conv.title,
              kind: conv.kind,
              color: conv.avatar_color,
              isEveryone: conv.is_everyone,
              message: null,
              fallback: conv.last_preview,
              unread: 0,
            },
            ...prev,
          ].slice(0, 6);
        });
        setIndex(0);
      }
      window.setTimeout(() => inputRef.current?.focus(), 160);
    };

    keep(onWindowEvent<Message>('bubble://incoming', handleIncoming));
    keep(onWindowEvent<string | null>('bubble://summon', (conv) => openFor(conv)));
    keep(onWindowEvent<string | null>('bubble://open', (conv) => openFor(conv)));

    keep(
      onCoreEvent((event) => {
        if (event.type === 'settings') {
          setSettings(event.settings);
        } else if (event.type === 'conversation_updated') {
          const conv = event.conversation;
          setConversations((list) => {
            const position = list.findIndex((item) => item.id === conv.id);
            return position >= 0
              ? list.map((item) => (item.id === conv.id ? conv : item))
              : [conv, ...list];
          });
        }
      }),
    );

    let moveTimer: number | undefined;
    let disposeMove: (() => void) | undefined;
    onMoved((position) => {
      if (moveTimer) window.clearTimeout(moveTimer);
      moveTimer = window.setTimeout(() => {
        void api.saveBubblePosition(Math.round(position.x), Math.round(position.y)).catch(() => undefined);
      }, 350);
    })
      .then((dispose) => {
        if (disposed) dispose();
        else disposeMove = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      disposers.forEach((dispose) => dispose());
      disposeMove?.();
      if (moveTimer) window.clearTimeout(moveTimer);
    };
  }, []);

  useEffect(() => {
    if (!ready || collapsed || !settings) return;
    const seconds = Math.max(3, settings.bubble.auto_hide_secs);
    const handle = window.setTimeout(() => {
      if (document.visibilityState === 'hidden') return;
      const focused = document.activeElement;
      const editing = focused instanceof HTMLInputElement || focused instanceof HTMLTextAreaElement;
      if (editing || draft.trim().length > 0) {
        setTick((value) => value + 1);
        return;
      }
      void api.hideBubble();
    }, seconds * 1000);
    return () => window.clearTimeout(handle);
  }, [ready, collapsed, settings, draft, tick, stack.length, index]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        void api.hideBubble();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const patchBubble = async (partial: Partial<Settings['bubble']>) => {
    const current = settingsRef.current;
    if (!current) return;
    const draftSettings = { ...current, bubble: { ...current.bubble, ...partial } };
    setSettings(draftSettings);
    try {
      const saved = await api.updateSettings(draftSettings);
      setSettings(saved);
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleCollapsed = (value: boolean) => {
    setCollapsed(value);
    setEmojiOpen(false);
    void patchBubble({ collapsed: value });
    if (!value) window.setTimeout(() => inputRef.current?.focus(), 200);
  };

  const current = stack[index] ?? null;
  const unreadTotal = conversations.reduce((sum, conv) => sum + conv.unread, 0);
  const badge = unreadTotal > 0 ? unreadTotal : stack.length;

  const send = async () => {
    const item = stackRef.current[indexRef.current];
    const text = draft.trim();
    if (!item || !text || busy) return;
    setBusy(true);
    setError(null);
    setDraft('');
    setEmojiOpen(false);
    try {
      const message = await api.sendText(item.conv, text);
      setStack((prev) => prev.map((entry) => (entry.conv === item.conv ? { ...entry, message } : entry)));
      flashSent();
      void api.markRead(item.conv).catch(() => undefined);
    } catch (err) {
      setDraft(text);
      raiseError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const sendFiles = async (files: File[]) => {
    const item = stackRef.current[indexRef.current];
    if (!item || files.length === 0) return;
    const limitMb = settingsRef.current?.max_file_mb ?? 2048;
    setBusy(true);
    for (const file of files) {
      if (file.size > limitMb * 1024 * 1024) {
        raiseError(`${file.name} is larger than the ${limitMb} MB limit.`);
        continue;
      }
      try {
        const data = await readAsBase64(file);
        const message = await api.sendFileBytes(
          item.conv,
          file.name,
          file.type || 'application/octet-stream',
          data,
        );
        setStack((prev) => prev.map((entry) => (entry.conv === item.conv ? { ...entry, message } : entry)));
        flashSent();
      } catch (err) {
        raiseError(String(err));
      }
    }
    setBusy(false);
    void api.markRead(item.conv).catch(() => undefined);
  };

  const sendPaths = useCallback(
    async (paths: string[]) => {
      const item = stackRef.current[indexRef.current];
      if (!item || paths.length === 0) return;
      setBusy(true);
      for (const path of paths) {
        try {
          const message = await api.sendFile(item.conv, path);
          setStack((prev) => prev.map((entry) => (entry.conv === item.conv ? { ...entry, message } : entry)));
          flashSent();
        } catch (err) {
          raiseError(String(err));
        }
      }
      setBusy(false);
    },
    [flashSent, raiseError],
  );

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | undefined;
    onDragDropFiles((paths) => void sendPaths(paths))
      .then((fn) => {
        if (disposed) fn();
        else dispose = fn;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      dispose?.();
    };
  }, [sendPaths]);

  const openInApp = () => {
    const item = stackRef.current[indexRef.current];
    void api
      .openMain(item?.conv ?? null)
      .then(() => api.hideBubble())
      .catch((err) => raiseError(String(err)));
  };

  const cycle = (direction: number) => {
    if (stack.length < 2) return;
    setIndex((value) => (value + direction + stack.length) % stack.length);
  };

  const close = () => {
    void api.hideBubble().catch(() => undefined);
  };

  if (collapsed) {
    const head = stack[0];
    const conv = head ? conversations.find((item) => item.id === head.conv) : conversations[0];
    return (
      <div className="flex h-full w-full items-center justify-center p-1">
        <button
          className="bubble-card relative flex h-full w-full items-center justify-center rounded-full"
          style={{ opacity: settings?.bubble.opacity ?? 0.96 }}
          onClick={() => toggleCollapsed(false)}
          aria-label="Expand quick reply"
          title="Open quick reply"
        >
          <Avatar
            name={conv?.title ?? 'LAN Chat'}
            color={head?.color ?? conv?.avatar_color ?? 0x4f5dff}
            size={58}
            kind={head?.kind === 'group' ? (head.isEveryone ? 'everyone' : 'group') : 'direct'}
          />
          {badge > 0 && (
            <span className="absolute -right-0.5 -top-0.5 flex h-5 min-w-[20px] items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-bold text-white ring-2 ring-[var(--bubble-bg)]">
              {badge > 99 ? '99+' : badge}
            </span>
          )}
        </button>
      </div>
    );
  }

  const headline = current
    ? current.kind === 'group' && current.message && !current.message.outgoing
      ? current.message.sender_name
      : current.title
    : 'LAN Chat';
  const subline = error ? (
    <span className="text-red-500">{error}</span>
  ) : sent ? (
    <span className="flex items-center gap-1 text-emerald-500">
      <IconCheck size={11} /> Sent
    </span>
  ) : current
    ? current.kind === 'group'
      ? current.isEveryone
        ? 'Everyone on the LAN'
        : `${current.title} · Group`
      : current.message
        ? 'New message'
        : 'Local network'
    : 'No conversations yet';

  return (
    <div className="flex h-full w-full items-stretch p-2">
      <div
        className="bubble-card flex min-w-0 flex-1 flex-col rounded-2xl"
        style={{ opacity: settings?.bubble.opacity ?? 0.96 }}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          void sendFiles(Array.from(event.dataTransfer.files));
        }}
      >
        <header
          className="flex shrink-0 items-center gap-2 px-2.5 pb-1 pt-2"
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            if ((event.target as HTMLElement).closest('button, input, textarea')) return;
            void startWindowDrag();
          }}
        >
          <Avatar
            name={headline}
            color={current ? current.color : 0x4f5dff}
            size={30}
            kind={
              current?.kind === 'group'
                ? current.isEveryone
                  ? 'everyone'
                  : 'group'
                : 'direct'
            }
          />
          <div className="min-w-0 flex-1 leading-tight">
            <div className="truncate text-[13px] font-semibold">{headline}</div>
            <div className="truncate text-[10px]" style={{ color: 'var(--muted)' }}>
              {subline}
            </div>
          </div>

          {stack.length > 1 && (
            <div className="flex shrink-0 items-center">
              <button
                className="icon-btn h-6 w-6"
                onClick={() => cycle(-1)}
                aria-label="Previous chat"
                title="Previous chat"
              >
                <IconChevronLeft size={13} />
              </button>
              <span className="w-6 text-center text-[10px] font-semibold tabular-nums" style={{ color: 'var(--muted)' }}>
                {index + 1}/{stack.length}
              </span>
              <button
                className="icon-btn h-6 w-6"
                onClick={() => cycle(1)}
                aria-label="Next chat"
                title="Next chat"
              >
                <IconChevronRight size={13} />
              </button>
            </div>
          )}

          <div className="flex shrink-0 items-center">
            <button
              className="icon-btn h-7 w-7"
              onClick={openInApp}
              title="Open in app"
              aria-label="Open in app"
            >
              <IconExternal size={15} />
            </button>
            <button
              className="icon-btn h-7 w-7"
              onClick={() => toggleCollapsed(true)}
              title="Collapse bubble"
              aria-label="Collapse bubble"
            >
              <IconMinimize size={15} />
            </button>
            <button className="icon-btn h-7 w-7" onClick={close} title="Close" aria-label="Close bubble">
              <IconX size={15} />
            </button>
          </div>
        </header>

        <div className="min-h-0 flex-1 overflow-hidden px-3 pb-1">
          {current?.message ? (
            <Preview message={current.message} />
          ) : (
            <p className="line-clamp-2 text-[12px] leading-snug" style={{ color: 'var(--muted)' }}>
              {current?.fallback || 'No messages yet — say hello.'}
            </p>
          )}
        </div>

        <div className="shrink-0 px-2 pb-2">
          <div
            className="relative flex items-center gap-0.5 rounded-xl border px-1 py-1 divider"
            style={{ background: 'var(--bg-soft)' }}
          >
            <button
              className="icon-btn h-7 w-7"
              onClick={() => fileRef.current?.click()}
              disabled={!current || busy}
              title="Attach a photo, video or file"
              aria-label="Attach"
            >
              <IconPaperclip size={16} />
            </button>
            <input
              ref={fileRef}
              type="file"
              multiple
              className="hidden"
              onChange={(event) => {
                void sendFiles(Array.from(event.target.files ?? []));
                event.target.value = '';
              }}
            />
            <input
              ref={inputRef}
              className="h-7 min-w-0 flex-1 bg-transparent px-1 text-[12px] outline-none"
              style={{ color: 'var(--text)' }}
              placeholder={current ? `Reply to ${firstName(current.title)}…` : 'No conversation'}
              value={draft}
              disabled={!current}
              onChange={(event) => {
                setDraft(event.target.value);
                setSent(false);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void send();
                }
              }}
              onPaste={(event) => {
                const files = Array.from(event.clipboardData.files);
                if (files.length > 0) {
                  event.preventDefault();
                  void sendFiles(files);
                }
              }}
            />
            <button
              className="icon-btn h-7 w-7"
              onClick={() => setEmojiOpen((value) => !value)}
              aria-label="Emoji"
              title="Emoji"
            >
              <IconSmile size={16} />
            </button>
            <button
              className="btn-primary h-7 min-w-[30px] !px-2"
              onClick={() => void send()}
              disabled={!current || !draft.trim() || busy}
              aria-label="Send reply"
              title="Send"
            >
              <IconSend size={14} />
            </button>
            {emojiOpen && (
              <div className="card absolute bottom-full right-0 mb-1 flex max-w-full gap-0.5 overflow-x-auto p-1 shadow-soft">
                {EMOJIS.map((emoji) => (
                  <button
                    key={emoji}
                    className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-base hover:bg-[var(--bg-hover)]"
                    onClick={() => {
                      setDraft((value) => value + emoji);
                      setEmojiOpen(false);
                      inputRef.current?.focus();
                    }}
                  >
                    {emoji}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Preview({ message }: { message: Message }) {
  const [thumb, setThumb] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (message.file_id && (message.kind === 'image' || message.kind === 'video')) {
      api
        .fileThumbnail(message.file_id)
        .then((value) => {
          if (alive) setThumb(value);
        })
        .catch(() => undefined);
    } else {
      setThumb(null);
    }
    return () => {
      alive = false;
    };
  }, [message.file_id, message.kind]);

  if (message.kind === 'text' || message.kind === 'system') {
    return (
      <p className="line-clamp-2 text-[12px] leading-snug" style={{ color: 'var(--text)' }}>
        {message.text}
      </p>
    );
  }

  const label =
    message.kind === 'image'
      ? 'Photo'
      : message.kind === 'video'
        ? 'Video'
        : message.kind === 'audio'
          ? 'Voice message'
          : 'File';

  return (
    <div className="flex items-center gap-2">
      <div
        className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg text-white"
        style={{ background: 'linear-gradient(140deg, var(--accent), var(--accent-2))' }}
      >
        {thumb ? (
          <img src={thumb} alt="" className="h-full w-full object-cover" draggable={false} />
        ) : message.kind === 'image' ? (
          <IconImage size={16} />
        ) : message.kind === 'video' ? (
          <IconVideo size={16} />
        ) : message.kind === 'audio' ? (
          <IconMic size={16} />
        ) : (
          <IconFile size={16} />
        )}
      </div>
      <div className="min-w-0">
        <div className="text-[12px] font-medium">{label}</div>
        <div className="truncate text-[11px]" style={{ color: 'var(--muted)' }}>
          {message.text || 'Tap to reply, open in app to download'}
        </div>
      </div>
    </div>
  );
}
