import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  api,
  ensureNotificationPermission,
  isTauri,
  onCoreEvent,
  onWindowEvent,
} from './api';
import type {
  Bootstrap,
  Capabilities,
  Conversation,
  FileProgress,
  Group,
  Identity,
  Message,
  Peer,
  Settings,
  Theme,
} from './types';
import { cls, readAsBase64 } from './utils';
import { BrandMark } from './components/Brand';
import { Sidebar } from './components/Sidebar';
import { ChatView } from './components/ChatView';
import { FirstRun } from './components/FirstRun';
import { SettingsModal } from './components/SettingsModal';
import { GroupModal } from './components/GroupModal';
import { SearchModal } from './components/SearchModal';
import { IconX } from './components/Icons';

const PAGE = 60;

type ModalState =
  | { kind: 'settings' }
  | { kind: 'group'; groupId: string | null }
  | { kind: 'search' }
  | null;

export function App() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [port, setPort] = useState(0);
  const [platform, setPlatform] = useState('');
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [configured, setConfigured] = useState(false);
  const [setupBusy, setSetupBusy] = useState(false);
  const [setupError, setSetupError] = useState<string | null>(null);

  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [msgConv, setMsgConv] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [typing, setTyping] = useState<{ conv: string; name: string } | null>(null);
  const [transfers, setTransfers] = useState<Record<string, FileProgress>>({});
  const [modal, setModal] = useState<ModalState>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [dark, setDark] = useState(() => document.documentElement.classList.contains('dark'));

  const activeRef = useRef<string | null>(null);
  const msgConvRef = useRef<string | null>(null);
  const typingSent = useRef(false);
  const typingTimer = useRef<number | undefined>(undefined);
  const toastTimer = useRef<number | undefined>(undefined);

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 5000);
  }, []);

  const applyTheme = useCallback((theme: Theme) => {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const isDark = theme === 'dark' || (theme === 'system' && prefersDark);
    document.documentElement.classList.toggle('dark', isDark);
    try {
      localStorage.setItem('lanchat-theme', theme);
    } catch {
      // Storage can be unavailable in private windows; theme still applies.
    }
    setDark(isDark);
  }, []);

  useEffect(() => {
    if (!settings || settings.theme !== 'system') return;
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => applyTheme('system');
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, [settings?.theme, applyTheme]);

  useEffect(() => {
    let alive = true;
    api
      .bootstrap()
      .then((data) => {
        if (!alive) return;
        setBoot(data);
        setIdentity(data.identity);
        setSettings(data.settings);
        setPeers(data.peers);
        setConversations([...data.conversations].sort((a, b) => b.last_ts - a.last_ts));
        setGroups(data.groups);
        setPort(data.port);
        setPlatform(data.platform);
        setCapabilities(data.capabilities);
        setConfigured(data.configured && Boolean(data.identity.display_name.trim()));
        applyTheme(data.settings.theme);

        let saved: string | null = null;
        try {
          saved = localStorage.getItem('lanchat-active');
        } catch {
          saved = null;
        }
        const preferred =
          saved && data.conversations.some((conv) => conv.id === saved)
            ? saved
            : window.innerWidth >= 768
              ? data.conversations[0]?.id ?? null
              : null;
        activeRef.current = preferred;
        setActiveId(preferred);
        void ensureNotificationPermission();
      })
      .catch((err) => {
        if (alive) setFatal(String(err));
      });
    return () => {
      alive = false;
    };
  }, [applyTheme]);

  const select = useCallback((id: string | null) => {
    activeRef.current = id;
    setActiveId(id);
    setTyping(null);
    typingSent.current = false;
    try {
      if (id) localStorage.setItem('lanchat-active', id);
      else localStorage.removeItem('lanchat-active');
    } catch {
      // Ignore storage errors.
    }
  }, []);

  useEffect(() => {
    if (!boot || !activeId) {
      msgConvRef.current = null;
      setMsgConv(null);
      setMessages([]);
      setHasMore(false);
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    api
      .messages(activeId, null, PAGE)
      .then((list) => {
        if (!alive) return;
        msgConvRef.current = activeId;
        setMessages(list);
        setMsgConv(activeId);
        setHasMore(list.length === PAGE);
        void api.markRead(activeId).catch(() => undefined);
        setConversations((current) =>
          current.map((conv) => (conv.id === activeId ? { ...conv, unread: 0, mentioned: false } : conv)),
        );
      })
      .catch((err) => {
        if (alive) showToast(String(err));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [boot, activeId, showToast]);

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

    keep(
      onCoreEvent((event) => {
        switch (event.type) {
          case 'peer': {
            const peer = event.peer;
            setPeers((list) => {
              const index = list.findIndex((item) => item.id === peer.id);
              return index >= 0
                ? list.map((item) => (item.id === peer.id ? peer : item))
                : [...list, peer];
            });
            setConversations((list) =>
              list.map((conv) =>
                conv.id === peer.id
                  ? {
                      ...conv,
                      title: peer.display_name || conv.title,
                      avatar_color: peer.avatar_color,
                      online: peer.online,
                    }
                  : conv,
              ),
            );
            break;
          }
          case 'peer_offline': {
            setPeers((list) =>
              list.map((peer) =>
                peer.id === event.id ? { ...peer, online: false, last_seen: Date.now() } : peer,
              ),
            );
            setConversations((list) =>
              list.map((conv) => (conv.id === event.id ? { ...conv, online: false } : conv)),
            );
            break;
          }
          case 'message': {
            const message = event.message;
            if (msgConvRef.current === message.conv) {
              setMessages((list) =>
                list.some((item) => item.id === message.id)
                  ? list.map((item) => (item.id === message.id ? message : item))
                  : [...list, message].sort((a, b) => a.ts - b.ts),
              );
            }
            if (message.conv === activeRef.current && !message.outgoing) {
              void api.markRead(message.conv).catch(() => undefined);
            }
            break;
          }
          case 'message_state': {
            setMessages((list) =>
              list.map((message) =>
                event.ids.includes(message.id) ? { ...message, state: event.state } : message,
              ),
            );
            break;
          }
          case 'typing': {
            if (event.conv !== activeRef.current) break;
            if (typingTimer.current) window.clearTimeout(typingTimer.current);
            if (event.active) {
              setTyping({ conv: event.conv, name: event.peer_name });
              typingTimer.current = window.setTimeout(() => setTyping(null), 5000);
            } else {
              setTyping(null);
            }
            break;
          }
          case 'group_updated': {
            const group = event.group;
            setGroups((list) => {
              const index = list.findIndex((item) => item.id === group.id);
              return index >= 0
                ? list.map((item) => (item.id === group.id ? group : item))
                : [...list, group];
            });
            setConversations((list) =>
              list.map((conv) =>
                conv.id === group.id
                  ? {
                      ...conv,
                      title: group.name,
                      avatar_color: group.avatar_color,
                      member_count: group.members.length,
                    }
                  : conv,
              ),
            );
            break;
          }
          case 'conversation_updated': {
            const conv = event.conversation;
            setConversations((list) => {
              const index = list.findIndex((item) => item.id === conv.id);
              const next = index >= 0 ? list.map((item) => (item.id === conv.id ? conv : item)) : [conv, ...list];
              return [...next].sort((a, b) => b.last_ts - a.last_ts);
            });
            break;
          }
          case 'transfer': {
            setTransfers((current) => ({ ...current, [event.file.transfer_id]: event.file }));
            break;
          }
          case 'error': {
            showToast(`${event.context}: ${event.message}`);
            break;
          }
          case 'network_changed': {
            void Promise.all([api.listPeers(), api.listConversations()])
              .then(([nextPeers, nextConvs]) => {
                setPeers(nextPeers);
                setConversations(nextConvs);
              })
              .catch(() => undefined);
            break;
          }
          case 'listening': {
            setPort(event.port);
            break;
          }
          case 'settings': {
            setSettings(event.settings);
            applyTheme(event.settings.theme);
            break;
          }
        }
      }),
    );

    keep(
      onWindowEvent<string>('app://open-conv', (conv) => {
        if (typeof conv === 'string') select(conv);
      }),
    );

    return () => {
      disposed = true;
      disposers.forEach((dispose) => dispose());
      if (typingTimer.current) window.clearTimeout(typingTimer.current);
    };
  }, [applyTheme, select, showToast]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        if (configured) setModal({ kind: 'search' });
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [configured]);

  const unreadTotal = useMemo(
    () => conversations.reduce((sum, conv) => sum + conv.unread, 0),
    [conversations],
  );

  useEffect(() => {
    document.title = unreadTotal > 0 ? `(${unreadTotal}) LAN Chat` : 'LAN Chat';
  }, [unreadTotal]);

  const active = useMemo(
    () => conversations.find((conv) => conv.id === activeId) ?? null,
    [conversations, activeId],
  );

  const upsertMessage = useCallback((message: Message) => {
    if (msgConvRef.current !== message.conv) return;
    setMessages((list) => {
      if (list.some((item) => item.id === message.id)) {
        return list.map((item) => (item.id === message.id ? message : item));
      }
      return [...list, message].sort((a, b) => a.ts - b.ts);
    });
  }, []);

  const handleSend = async (text: string) => {
    const conv = activeRef.current;
    if (!conv) return;
    try {
      const message = await api.sendText(conv, text);
      upsertMessage(message);
    } catch (err) {
      showToast(`Could not send: ${String(err)}`);
    }
  };

  const handleAttach = async (files: File[]) => {
    const conv = activeRef.current;
    if (!conv) return;
    const limitMb = settings?.max_file_mb ?? 2048;
    const limit = limitMb * 1024 * 1024;
    for (const file of files) {
      if (file.size > limit) {
        showToast(`${file.name} is larger than the ${limitMb} MB limit in Settings.`);
        continue;
      }
      try {
        const data = await readAsBase64(file);
        const message = await api.sendFileBytes(
          conv,
          file.name,
          file.type || 'application/octet-stream',
          data,
        );
        upsertMessage(message);
      } catch (err) {
        showToast(`Could not send ${file.name}: ${String(err)}`);
      }
    }
  };

  const handleTyping = (isActive: boolean) => {
    const conv = activeRef.current;
    if (!conv) return;
    if (typingSent.current === isActive) return;
    typingSent.current = isActive;
    api.setTyping(conv, isActive).catch(() => undefined);
  };

  const loadMore = async () => {
    if (!activeId || loading || messages.length === 0) return;
    setLoading(true);
    try {
      const older = await api.messages(activeId, messages[0].ts, PAGE);
      setMessages((current) => {
        const seen = new Set(current.map((message) => message.id));
        return [...older.filter((message) => !seen.has(message.id)), ...current];
      });
      setHasMore(older.length === PAGE);
    } catch (err) {
      showToast(String(err));
    } finally {
      setLoading(false);
    }
  };

  const toggleMute = async () => {
    if (!active) return;
    try {
      await api.setMuted(active.id, !active.muted);
    } catch (err) {
      showToast(String(err));
    }
  };

  const patchSettings = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const previous = settings;
    const draft = { ...settings, ...patch };
    setSettings(draft);
    if (patch.theme) applyTheme(patch.theme);
    try {
      const saved = await api.updateSettings(draft);
      setSettings(saved);
    } catch (err) {
      setSettings(previous);
      if (patch.theme) applyTheme(previous.theme);
      showToast(String(err));
    }
  };

  const saveSettings = async (draft: Settings, displayName: string, avatarColor: number) => {
    const trimmed = displayName.trim();
    if (identity && (trimmed !== identity.display_name || avatarColor !== identity.avatar_color)) {
      const updated = await api.saveIdentity(trimmed, avatarColor);
      setIdentity(updated);
    }
    const saved = await api.updateSettings(draft);
    setSettings(saved);
    applyTheme(saved.theme);
    if (!saved.bubble.enabled) await api.hideBubble().catch(() => undefined);
  };

  const resetBubblePosition = async () => {
    try {
      await api.resetBubblePosition();
      setSettings((current) =>
        current ? { ...current, bubble: { ...current.bubble, x: null, y: null } } : current,
      );
    } catch (err) {
      showToast(String(err));
    }
  };

  const finishSetup = async (name: string, color: number, passphrase: string | null) => {
    if (!settings) return;
    setSetupBusy(true);
    setSetupError(null);
    try {
      const updated = await api.saveIdentity(name, color);
      const saved = await api.updateSettings({ ...settings, passphrase });
      setIdentity(updated);
      setSettings(saved);
      setConfigured(true);
      applyTheme(saved.theme);
      const [nextPeers, nextConvs, nextGroups] = await Promise.all([
        api.listPeers(),
        api.listConversations(),
        api.listGroups(),
      ]);
      setPeers(nextPeers);
      setConversations(nextConvs);
      setGroups(nextGroups);
      if (!activeRef.current && nextConvs[0]) select(nextConvs[0].id);
      void ensureNotificationPermission();
    } catch (err) {
      setSetupError(String(err));
    } finally {
      setSetupBusy(false);
    }
  };

  const refreshGroups = async () => {
    const [nextGroups, nextConvs] = await Promise.all([api.listGroups(), api.listConversations()]);
    setGroups(nextGroups);
    setConversations(nextConvs);
  };

  const createGroup = async (name: string, members: string[]) => {
    const group = await api.createGroup(name, members);
    await refreshGroups();
    select(group.id);
    setModal(null);
  };

  const addMember = async (groupId: string, peer: string) => {
    await api.addGroupMember(groupId, peer);
    await refreshGroups();
  };

  const removeMember = async (groupId: string, peer: string) => {
    await api.removeGroupMember(groupId, peer);
    await refreshGroups();
  };

  const renameGroup = async (groupId: string, name: string) => {
    await api.renameGroup(groupId, name);
    setGroups((list) => list.map((group) => (group.id === groupId ? { ...group, name } : group)));
    setConversations((list) =>
      list.map((conv) => (conv.id === groupId ? { ...conv, title: name } : conv)),
    );
  };

  const leaveGroup = async (groupId: string) => {
    await api.leaveGroup(groupId);
    setGroups((list) => list.filter((group) => group.id !== groupId));
    setConversations((list) => list.filter((conv) => conv.id !== groupId));
    if (activeRef.current === groupId) select(null);
    setModal(null);
  };

  const acceptFile = (id: string) => {
    api.respondFile(id, true).catch((err) => showToast(String(err)));
  };

  const declineFile = (id: string) => {
    api.respondFile(id, false).catch((err) => showToast(String(err)));
  };

  const openBubble = () => {
    api.showBubble(activeId).catch((err) => showToast(String(err)));
  };

  const modalGroup =
    modal?.kind === 'group' && modal.groupId
      ? groups.find((group) => group.id === modal.groupId) ?? null
      : null;

  if (fatal) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
        <BrandMark size={56} />
        <h1 className="text-lg font-semibold">LAN Chat could not start</h1>
        <p className="max-w-md text-sm" style={{ color: 'var(--muted)' }}>
          {fatal}
        </p>
        <button className="btn-primary mt-2" onClick={() => window.location.reload()}>
          Try again
        </button>
      </div>
    );
  }

  if (!boot) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <BrandMark size={56} />
        <p className="text-sm" style={{ color: 'var(--muted)' }}>
          Starting LAN discovery…
        </p>
      </div>
    );
  }

  if (!configured) {
    return (
      <div className="min-h-full overflow-y-auto">
        <FirstRun busy={setupBusy} error={setupError} onDone={finishSetup} />
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-hidden" style={{ background: 'var(--bg)' }}>
      <div
        className={cls(
          'h-full w-full shrink-0 md:block md:w-[318px] lg:w-[340px]',
          activeId ? 'hidden' : 'block',
        )}
      >
        {identity && settings && (
          <Sidebar
            identity={identity}
            conversations={conversations}
            activeId={activeId}
            onlineCount={peers.filter((peer) => peer.online).length}
            port={port}
            dnd={settings.dnd}
            dark={dark}
            tauri={isTauri}
            onSelect={select}
            onSettings={() => setModal({ kind: 'settings' })}
            onNewGroup={() => setModal({ kind: 'group', groupId: null })}
            onToggleDnd={() => void patchSettings({ dnd: !settings.dnd })}
            onToggleTheme={() => void patchSettings({ theme: dark ? 'light' : 'dark' })}
          />
        )}
      </div>

      <main className={cls('h-full min-w-0 flex-1 md:block', activeId ? 'block' : 'hidden')}>
        <ChatView
          conv={active}
          meId={identity?.peer_id ?? ''}
          peers={peers}
          messages={msgConv === activeId ? messages : []}
          typingName={typing && typing.conv === activeId ? typing.name : null}
          transfers={transfers}
          hasMore={hasMore}
          loading={loading}
          onSend={handleSend}
          onAttach={handleAttach}
          onTyping={handleTyping}
          onLoadMore={() => void loadMore()}
          onMute={() => void toggleMute()}
          onManage={() =>
            active && active.kind === 'group'
              ? setModal({ kind: 'group', groupId: active.id })
              : undefined
          }
          onSearch={() => setModal({ kind: 'search' })}
          onOpenBubble={openBubble}
          onBack={() => select(null)}
          onAccept={acceptFile}
          onDecline={declineFile}
        />
      </main>

      {modal?.kind === 'settings' && settings && identity && capabilities && (
        <SettingsModal
          identity={identity}
          settings={settings}
          capabilities={capabilities}
          platform={platform}
          onClose={() => setModal(null)}
          onSave={saveSettings}
          onResetBubble={resetBubblePosition}
        />
      )}

      {modal?.kind === 'group' && identity && (modal.groupId === null || modalGroup) && (
        <GroupModal
          group={modalGroup}
          peers={peers}
          meId={identity.peer_id}
          onClose={() => setModal(null)}
          onCreate={createGroup}
          onRename={renameGroup}
          onAdd={addMember}
          onRemove={removeMember}
          onLeave={leaveGroup}
        />
      )}

      {modal?.kind === 'search' && (
        <SearchModal
          conversations={conversations}
          onClose={() => setModal(null)}
          onOpen={select}
        />
      )}

      {toast && (
        <div className="fixed bottom-5 left-1/2 z-[70] flex max-w-[92vw] -translate-x-1/2 items-center gap-3 rounded-xl bg-slate-900 px-4 py-2.5 text-sm text-white shadow-xl">
          <span className="min-w-0">{toast}</span>
          <button
            className="text-white/60 hover:text-white"
            onClick={() => setToast(null)}
            aria-label="Dismiss"
          >
            <IconX size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
