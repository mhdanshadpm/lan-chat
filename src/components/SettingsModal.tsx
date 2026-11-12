import { useState, type ReactNode } from 'react';
import type { Capabilities, Corner, Identity, Settings, Theme } from '../types';
import { Avatar } from './Avatar';
import { Modal } from './Modal';
import { Switch } from './Switch';
import { cls } from '../utils';

const PALETTE = [
  0xe06c75, 0xf29e6e, 0xf5d06b, 0x8bd49a, 0x6bd3c9,
  0x74aaf7, 0x9b8cf5, 0xe08cf5, 0xf58cc0, 0x9aa6b4,
];

const CORNERS: Array<[Corner, string]> = [
  ['top_left', 'Top left'],
  ['top_right', 'Top right'],
  ['bottom_left', 'Bottom left'],
  ['bottom_right', 'Bottom right'],
];

interface Props {
  identity: Identity;
  settings: Settings;
  capabilities: Capabilities;
  platform: string;
  onClose: () => void;
  onSave: (settings: Settings, displayName: string, avatarColor: number) => Promise<void>;
  onResetBubble: () => Promise<void>;
}

function Section({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="border-b py-5 first:pt-1 last:border-0 divider">
      <h3 className="text-sm font-semibold">{title}</h3>
      {description && (
        <p className="mt-1 text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
          {description}
        </p>
      )}
      <div className="mt-3 space-y-3">{children}</div>
    </section>
  );
}

function Row({ title, description, control }: { title: string; description?: string; control: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {description && (
          <p className="mt-0.5 text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
            {description}
          </p>
        )}
      </div>
      <div className="shrink-0 pt-0.5">{control}</div>
    </div>
  );
}

export function SettingsModal({
  identity,
  settings,
  capabilities,
  platform,
  onClose,
  onSave,
  onResetBubble,
}: Props) {
  const [draft, setDraft] = useState<Settings>(() => structuredClone(settings));
  const [displayName, setDisplayName] = useState(identity.display_name);
  const [avatarColor, setAvatarColor] = useState(identity.avatar_color);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const patch = (values: Partial<Settings>) => setDraft((current) => ({ ...current, ...values }));
  const patchBubble = (values: Partial<Settings['bubble']>) =>
    setDraft((current) => ({ ...current, bubble: { ...current.bubble, ...values } }));

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSave(draft, displayName, avatarColor);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Settings"
      subtitle="Everything is stored locally on this device."
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn-primary" onClick={() => void save()} disabled={busy || !displayName.trim()}>
            {busy ? 'Saving…' : 'Save changes'}
          </button>
        </>
      }
    >
      {error && <p className="mb-3 rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-500">{error}</p>}

      <Section title="You" description="Shown to other people on the local network.">
        <div className="flex items-center gap-3">
          <Avatar name={displayName || 'You'} color={avatarColor} size={48} />
          <input
            className="field"
            value={displayName}
            maxLength={64}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Display name"
          />
        </div>
        <div className="flex flex-wrap gap-2">
          {PALETTE.map((value) => (
            <button
              key={value}
              onClick={() => setAvatarColor(value)}
              aria-label={`Avatar colour ${value.toString(16)}`}
              className={cls('h-7 w-7 rounded-full transition-transform hover:scale-110', avatarColor === value && 'ring-2 ring-[var(--accent)] ring-offset-2')}
              style={{ background: `#${value.toString(16).padStart(6, '0')}` }}
            />
          ))}
        </div>
      </Section>

      <Section title="Appearance">
        <Row
          title="Theme"
          description="Follow the system or force light/dark."
          control={
            <div className="flex rounded-lg p-0.5" style={{ background: 'var(--bg-soft)' }}>
              {(['system', 'light', 'dark'] as Theme[]).map((theme) => (
                <button
                  key={theme}
                  className={cls(
                    'rounded-md px-2.5 py-1 text-xs font-semibold capitalize',
                    draft.theme === theme ? 'bg-[var(--bg-elevated)] shadow-sm' : 'opacity-70',
                  )}
                  onClick={() => patch({ theme })}
                >
                  {theme}
                </button>
              ))}
            </div>
          }
        />
      </Section>

      <Section
        title="Quick-reply bubble"
        description={
          capabilities.bubble_over_fullscreen
            ? 'The floating bubble appears above other windows, across Spaces and over fullscreen apps.'
            : 'The floating bubble appears above other windows where the OS allows it.'
        }
      >
        <Row
          title="Enable bubble"
          description="Show new messages in a floating quick-reply popup."
          control={<Switch checked={draft.bubble.enabled} onChange={(enabled) => patchBubble({ enabled })} />}
        />
        <Row
          title="Pop for every message"
          description="Keep showing the bubble even when it is already open."
          control={
            <Switch
              checked={draft.bubble.show_on_every_message}
              onChange={(show_on_every_message) => patchBubble({ show_on_every_message })}
            />
          }
        />
        <Row
          title="Auto-hide delay"
          description="Seconds of inactivity before the bubble hides."
          control={
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={3}
                max={120}
                className="field w-20 py-1.5 text-center"
                value={draft.bubble.auto_hide_secs}
                onChange={(event) =>
                  patchBubble({ auto_hide_secs: Math.max(3, Math.min(120, Number(event.target.value) || 8)) })
                }
              />
              <span className="text-xs" style={{ color: 'var(--muted)' }}>
                sec
              </span>
            </div>
          }
        />
        <div>
          <span className="label">Corner position</span>
          <div className="grid grid-cols-2 gap-2">
            {CORNERS.map(([value, label]) => (
              <button
                key={value}
                onClick={() => patchBubble({ corner: value, x: null, y: null })}
                className={cls('btn-soft justify-start text-xs', draft.bubble.corner === value && 'ring-2 ring-[var(--accent)]')}
              >
                {label}
              </button>
            ))}
          </div>
          <p className="mt-1.5 text-[11px]" style={{ color: 'var(--muted)' }}>
            Drag the bubble to place it anywhere; the position is remembered until you reset it.
          </p>
        </div>
        <Row
          title="Opacity"
          description={`${Math.round(draft.bubble.opacity * 100)}%`}
          control={
            <input
              type="range"
              min={55}
              max={100}
              value={Math.round(draft.bubble.opacity * 100)}
              onChange={(event) => patchBubble({ opacity: Number(event.target.value) / 100 })}
              className="w-40 accent-[var(--accent)]"
            />
          }
        />
        <Row
          title="Start collapsed"
          description="Show a small avatar with an unread count until clicked."
          control={<Switch checked={draft.bubble.collapsed} onChange={(collapsed) => patchBubble({ collapsed })} />}
        />
        <Row
          title="Hide while the app is focused"
          description="Only pop the bubble when you are in another app."
          control={
            <Switch
              checked={draft.pause_bubble_when_app_focused}
              onChange={(pause_bubble_when_app_focused) => patch({ pause_bubble_when_app_focused })}
            />
          }
        />
        <Row
          title="Reset bubble position"
          description="Return the bubble to the selected corner."
          control={
            <button
              className="btn-soft text-xs"
              onClick={() => {
                void onResetBubble();
                patchBubble({ x: null, y: null });
              }}
            >
              Reset
            </button>
          }
        />
        {capabilities.global_hotkey && (
          <p className="text-[11px]" style={{ color: 'var(--muted)' }}>
            Global hotkey: <span className="chip">Ctrl/Cmd + Shift + Space</span> opens the bubble for the last
            active chat.
          </p>
        )}
      </Section>

      <Section title="Notifications">
        <Row
          title="Do not disturb"
          description="Silence the bubble, notifications and sound. Messages are still saved."
          control={<Switch checked={draft.dnd} onChange={(dnd) => patch({ dnd })} />}
        />
        <Row
          title="Notification sound"
          description={capabilities.native_sound ? 'Play the system notification sound.' : 'Sound is not available on this platform.'}
          control={
            <Switch
              checked={draft.notification_sound}
              disabled={!capabilities.native_sound}
              onChange={(notification_sound) => patch({ notification_sound })}
            />
          }
        />
        <Row
          title="Launch at login"
          description="Start LAN Chat when you sign in."
          control={
            <Switch
              checked={draft.launch_at_login}
              disabled={!capabilities.autostart}
              onChange={(launch_at_login) => patch({ launch_at_login })}
            />
          }
        />
      </Section>

      <Section title="Files">
        <Row
          title="Auto-accept incoming files"
          description="Files at or below this size download without asking."
          control={
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={0}
                max={2048}
                className="field w-20 py-1.5 text-center"
                value={draft.auto_accept_files_mb}
                onChange={(event) =>
                  patch({ auto_accept_files_mb: Math.max(0, Number(event.target.value) || 0) })
                }
              />
              <span className="text-xs" style={{ color: 'var(--muted)' }}>
                MB
              </span>
            </div>
          }
        />
        <Row
          title="Maximum file size"
          description="Larger files are rejected before transfer."
          control={
            <div className="flex items-center gap-2">
              <input
                type="number"
                min={1}
                max={8192}
                className="field w-20 py-1.5 text-center"
                value={draft.max_file_mb}
                onChange={(event) =>
                  patch({ max_file_mb: Math.max(1, Number(event.target.value) || 1) })
                }
              />
              <span className="text-xs" style={{ color: 'var(--muted)' }}>
                MB
              </span>
            </div>
          }
        />
        <p className="text-[11px] leading-relaxed" style={{ color: 'var(--muted)' }}>
          Transfers are chunked and resume automatically after a disconnect. Image and video
          thumbnails are generated locally before the offer is sent.
        </p>
      </Section>

      <Section title="Privacy & network">
        <Row
          title="LAN passphrase"
          description="When set, every frame is encrypted. All peers need the same passphrase."
          control={
            <button className="btn-soft text-xs" onClick={() => setShowPassphrase((value) => !value)}>
              {showPassphrase ? 'Hide' : draft.passphrase ? 'Change' : 'Add'}
            </button>
          }
        />
        {showPassphrase && (
          <input
            type="password"
            className="field"
            placeholder="Shared passphrase (leave empty to disable)"
            value={draft.passphrase ?? ''}
            onChange={(event) => patch({ passphrase: event.target.value || null })}
          />
        )}
        {draft.passphrase !== settings.passphrase && (
          <p className="text-[11px] text-amber-500">
            Changing the passphrase disconnects current peers and restarts discovery.
          </p>
        )}
        <Row
          title="Read receipts"
          description="Tell peers when you have read their messages."
          control={<Switch checked={draft.read_receipts} onChange={(read_receipts) => patch({ read_receipts })} />}
        />
        <Row
          title="Typing indicators"
          description="Share when you are typing."
          control={
            <Switch
              checked={draft.typing_indicators}
              onChange={(typing_indicators) => patch({ typing_indicators })}
            />
          }
        />
        <Row
          title="Discovery port"
          description="mDNS uses _lanchat._tcp; UDP broadcast falls back to this port."
          control={<span className="chip">{draft.discovery_port} (restart to change)</span>}
        />
      </Section>

      <p className="pt-3 text-center text-[11px]" style={{ color: 'var(--muted)' }}>
        LAN Chat 0.1.0 · {platform}
      </p>
    </Modal>
  );
}
