import { useState } from 'react';
import type { Group, Peer } from '../types';
import { Avatar } from './Avatar';
import { Modal } from './Modal';
import { IconCheck, IconPlus, IconTrash, IconX } from './Icons';
import { cls } from '../utils';

interface Props {
  group: Group | null;
  peers: Peer[];
  meId: string;
  onClose: () => void;
  onCreate: (name: string, members: string[]) => Promise<void>;
  onRename: (id: string, name: string) => Promise<void>;
  onAdd: (id: string, peer: string) => Promise<void>;
  onRemove: (id: string, peer: string) => Promise<void>;
  onLeave: (id: string) => Promise<void>;
}

export function GroupModal({
  group,
  peers,
  meId,
  onClose,
  onCreate,
  onRename,
  onAdd,
  onRemove,
  onLeave,
}: Props) {
  const isNew = !group;
  const readOnly = Boolean(group?.is_default);
  const [name, setName] = useState(group?.name ?? '');
  const [selected, setSelected] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmLeave, setConfirmLeave] = useState(false);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const toggleSelected = (id: string) => {
    setSelected((current) =>
      current.includes(id) ? current.filter((value) => value !== id) : [...current, id],
    );
  };

  const available = group ? peers.filter((peer) => !group.members.includes(peer.id)) : peers;

  const footer = isNew ? (
    <>
      <button className="btn-ghost" onClick={onClose}>
        Cancel
      </button>
      <button
        className="btn-primary"
        disabled={busy || !name.trim()}
        onClick={() => void run(async () => onCreate(name.trim(), selected))}
      >
        {busy ? 'Creating…' : 'Create group'}
      </button>
    </>
  ) : confirmLeave && group ? (
    <>
      <span className="mr-auto text-xs" style={{ color: 'var(--muted)' }}>
        Leave {group.name}? History stays on this device.
      </span>
      <button className="btn-ghost" onClick={() => setConfirmLeave(false)}>
        Stay
      </button>
      <button
        className="btn-danger"
        disabled={busy}
        onClick={() =>
          void run(async () => {
            await onLeave(group.id);
            onClose();
          })
        }
      >
        Leave group
      </button>
    </>
  ) : (
    <>
      {!readOnly && group && (
        <button className="btn-ghost mr-auto text-red-500" onClick={() => setConfirmLeave(true)}>
          <IconTrash size={15} /> Leave group
        </button>
      )}
      <button className="btn-primary" onClick={onClose}>
        Done
      </button>
    </>
  );

  return (
    <Modal
      title={isNew ? 'New group' : group?.is_default ? 'Everyone' : 'Manage group'}
      subtitle={
        isNew
          ? 'Groups are local to the LAN and sync to their members.'
          : readOnly
            ? 'Every device on this network is a member of Everyone.'
            : `${group?.members.length ?? 0} ${group?.members.length === 1 ? 'member' : 'members'}`
      }
      onClose={onClose}
      footer={footer}
    >
      {error && <p className="mb-3 rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-500">{error}</p>}

      {isNew || (!readOnly && group) ? (
        <div className="mb-5">
          <label className="label" htmlFor="group-name">
            Group name
          </label>
          <div className="flex gap-2">
            <input
              id="group-name"
              className="field"
              value={name}
              maxLength={64}
              placeholder="e.g. Studio, Living room, Shift handover"
              onChange={(event) => setName(event.target.value)}
              autoFocus={isNew}
            />
            {!isNew && group && (
              <button
                className="btn-soft shrink-0"
                disabled={busy || !name.trim() || name.trim() === group.name}
                onClick={() =>
                  void run(async () => {
                    await onRename(group.id, name.trim());
                  })
                }
              >
                Rename
              </button>
            )}
          </div>
        </div>
      ) : null}

      <div>
        <span className="label">{isNew ? 'Invite people' : 'Members'}</span>
        <div className="space-y-1.5">
          {group && (
            <div className="flex items-center gap-3 rounded-xl px-2 py-1.5" style={{ background: 'var(--bg-soft)' }}>
              <Avatar name="You" color={0x9aa6b4} size={30} />
              <span className="min-w-0 flex-1 truncate text-sm font-medium">You</span>
              <span className="chip">you</span>
            </div>
          )}
          {group &&
            group.members
              .filter((id) => id !== meId)
              .map((id) => {
                const peer = peers.find((item) => item.id === id);
                return (
                  <div key={id} className="flex items-center gap-3 rounded-xl px-2 py-1.5">
                    <Avatar
                      name={peer?.display_name ?? id.slice(0, 8)}
                      color={peer?.avatar_color ?? 0x9aa6b4}
                      size={30}
                      online={peer ? peer.online : false}
                    />
                    <span className="min-w-0 flex-1 truncate text-sm">
                      {peer?.display_name ?? id.slice(0, 8)}
                    </span>
                    {!readOnly && (
                      <button
                        className="icon-btn h-8 w-8 text-red-500"
                        title="Remove from group"
                        aria-label="Remove from group"
                        disabled={busy}
                        onClick={() => void run(() => onRemove(group.id, id))}
                      >
                        <IconX size={16} />
                      </button>
                    )}
                  </div>
                );
              })}

          {isNew &&
            peers.map((peer) => {
              const checked = selected.includes(peer.id);
              return (
                <button
                  key={peer.id}
                  type="button"
                  onClick={() => toggleSelected(peer.id)}
                  className={cls(
                    'flex w-full items-center gap-3 rounded-xl px-2 py-1.5 text-left transition-colors',
                    checked ? 'bg-[var(--accent-soft)]' : 'hover:bg-[var(--bg-hover)]',
                  )}
                >
                  <Avatar name={peer.display_name} color={peer.avatar_color} size={30} online={peer.online} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{peer.display_name}</span>
                    <span className="block truncate text-[11px]" style={{ color: 'var(--muted)' }}>
                      {peer.online ? 'Online' : peer.device_name || 'Offline'}
                    </span>
                  </span>
                  <span
                    className="flex h-5 w-5 items-center justify-center rounded-md border"
                    style={{
                      borderColor: checked ? 'var(--accent)' : 'var(--border)',
                      background: checked ? 'var(--accent)' : 'transparent',
                      color: '#fff',
                    }}
                  >
                    {checked && <IconCheck size={13} />}
                  </span>
                </button>
              );
            })}
        </div>
      </div>

      {!isNew && !readOnly && (
        <div className="mt-5">
          <span className="label">Add people</span>
          {available.length === 0 ? (
            <p className="text-xs" style={{ color: 'var(--muted)' }}>
              Everyone visible on this network is already a member.
            </p>
          ) : (
            <div className="space-y-1.5">
              {available.map((peer) => (
                <div key={peer.id} className="flex items-center gap-3 rounded-xl px-2 py-1.5">
                  <Avatar name={peer.display_name} color={peer.avatar_color} size={30} online={peer.online} />
                  <span className="min-w-0 flex-1 truncate text-sm">{peer.display_name}</span>
                  <button
                    className="btn-soft px-2 py-1 text-xs"
                    disabled={busy}
                    onClick={() => group && void run(() => onAdd(group.id, peer.id))}
                  >
                    <IconPlus size={13} /> Add
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {readOnly && (
        <p className="mt-4 text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
          Everyone is created automatically when peers appear on the network. You cannot be removed
          from it, but you can mute it from the conversation header.
        </p>
      )}
    </Modal>
  );
}
